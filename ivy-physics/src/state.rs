use itertools::Itertools;
use ivy_core::components::{
    angular_velocity, is_static, is_trigger, mass, velocity, world_transform,
};
use std::collections::{btree_map::Entry, BTreeMap, BTreeSet, HashSet};

use flax::{
    fetch::{entity_refs, Satisfied},
    CommandBuffer, Component, Entity, Fetch, FetchExt, Mutable, Opt, OptOr, Query, QueryBorrow,
    World,
};
use glam::{Mat4, Vec3};
use ivy_collision::{
    body::{Body, BodyIndex, ContactIndex},
    components::{body_index, collider, collider_offset},
    island::{Island, IslandContactIter, Islands},
    util::IndexedRange,
    BvhNode, Collider, CollisionTree, EntityPayload, IntersectionGenerator, NodeIndex, NodeState,
    PersistentContact, PersistentContactPoint, Shape, TransformedShape,
};
use slotmap::{Key, SlotMap};

use crate::response::{SimulationBody, Solver, SolverConfiguration};

#[derive(Default)]
pub struct PhysicsStateConfiguration {
    solver: SolverConfiguration,
}

pub struct PhysicsState {
    bodies: SlotMap<BodyIndex, Body>,
    solver: Solver,
    tree: CollisionTree,
    islands: Islands,
    generation: u32,
    intersection_generator: IntersectionGenerator,
    contact_map: BTreeMap<(BodyIndex, IndexedRange<BodyIndex>), ContactIndex>,
    contacts: SlotMap<ContactIndex, PersistentContact>,

    dirty_bodies: HashSet<BodyIndex>,
    removed_bodies: HashSet<BodyIndex>,
}

impl PhysicsState {
    pub fn new(configuration: &PhysicsStateConfiguration, dt: f32) -> Self {
        Self {
            bodies: Default::default(),
            tree: CollisionTree::new(BvhNode::default()),
            solver: Solver::new(configuration.solver, dt),
            islands: Islands::new(),
            intersection_generator: IntersectionGenerator::new(),
            generation: 0,
            contact_map: Default::default(),
            contacts: Default::default(),
            dirty_bodies: Default::default(),
            removed_bodies: Default::default(),
        }
    }

    pub fn islands(&self) -> slotmap::secondary::Iter<BodyIndex, Island> {
        self.islands.islands().iter()
    }

    pub fn island_contacts(&self, island: &Island) -> IslandContactIter {
        island.contacts(&self.contacts)
    }

    pub fn contacts(&self) -> slotmap::basic::Iter<ContactIndex, PersistentContact> {
        self.contacts.iter()
    }

    pub fn body(&self, body_index: BodyIndex) -> &Body {
        &self.bodies[body_index]
    }

    pub fn register_bodies(&mut self, world: &World, cmd: &mut CommandBuffer) {
        let mut query = Query::new((entity_refs(), ObjectQuery::new())).without(body_index());

        for (entity, q) in query.borrow(world).iter() {
            let offset = *q.offset;
            let transform = *q.transform * offset;

            let bounds = q.collider.bounding_box(transform);
            let extended_bounds = bounds.expand(*q.velocity * 0.1);

            let body = Body {
                id: entity.id(),
                bounds,
                extended_bounds,
                transform,
                is_trigger: q.is_trigger,
                state: if q.is_static {
                    NodeState::Static
                } else {
                    NodeState::Dynamic
                },
                movable: q.mass.map(|v| v.is_normal()).unwrap_or(false),
                collider: q.collider.clone(),
                node: NodeIndex::null(),
                island: BodyIndex::null(),
                next_body: BodyIndex::null(),
                prev_body: BodyIndex::null(),
            };

            let index = self.bodies.insert(body);
            self.tree.insert_body(&mut self.bodies, index);
            self.islands.create_island(index);
            if q.is_static {
                self.islands.mark_static(index);
            }

            self.islands.link_body(&mut self.bodies, index);
            self.solver
                .add_body(index, SimulationBody::from_entity(&entity).unwrap());

            // let tree_index = self.insert_body(id, body);

            // BvhNode::update_bounds(self.root, &mut self.nodes, &self.bodies);
            cmd.set(entity.id(), body_index(), index);
        }
    }

    pub(crate) fn simulation_body_mut(&mut self, index: BodyIndex) -> &mut SimulationBody {
        &mut self.solver.bodies_mut()[index]
    }

    pub fn update_bodies<I>(&mut self, data: I)
    where
        I: Iterator<Item = (BodyIndex, Mat4, Vec3)>,
    {
        for (body_index, transform, velocity) in data {
            let body = &mut self.bodies[body_index];
            body.transform = transform;
            body.bounds = body.collider.bounding_box(transform);
            body.extended_bounds = body.bounds.expand(velocity.abs() * 0.1);

            self.tree.update(body_index, body).unwrap();
        }

        self.tree.refit(&mut self.bodies);
    }

    pub fn generate_contacts(&mut self) {
        let mut result = Vec::new();
        self.tree
            .check_collisions(&self.bodies, &mut result)
            .unwrap();

        self.generation += 1;

        for (mut a, mut b) in result {
            // ensure stable indexing and links
            if a > b {
                std::mem::swap(&mut a, &mut b);
            }

            let a_body = &self.bodies[a];
            let b_body = &self.bodies[b];

            let Some(contact) = self.intersection_generator.intersect(
                &TransformedShape::new(&a_body.collider, a_body.transform),
                &TransformedShape::new(&b_body.collider, b_body.transform),
            ) else {
                continue;
            };

            let local_anchors = (
                a_body.transform.inverse().transform_point3(contact.point_a),
                b_body.transform.inverse().transform_point3(contact.point_b),
            );

            let contact_point = PersistentContactPoint::new(
                (contact.point_a, contact.point_b),
                local_anchors,
                contact.depth,
                contact.normal,
            );

            match self.contact_map.entry((a, IndexedRange::Exact(b))) {
                Entry::Vacant(slot) => {
                    // new island
                    let contact = PersistentContact::new(
                        EntityPayload {
                            entity: a_body.id,
                            is_trigger: false,
                            state: a_body.state,
                            body: a,
                        },
                        EntityPayload {
                            entity: b_body.id,
                            is_trigger: false,
                            state: b_body.state,
                            body: b,
                        },
                        contact_point,
                        self.generation,
                    );

                    let id = self.contacts.insert(contact);
                    slot.insert(id);

                    self.islands.link(&mut self.contacts, id);

                    assert!(!self.contact_map.contains_key(&(b, IndexedRange::Exact(a))));

                    self.contact_map.insert((b, IndexedRange::Exact(a)), id);
                }
                Entry::Occupied(v) => {
                    let &contact_index = v.get();
                    let v = &mut self.contacts[contact_index];

                    v.add_point(contact_point);
                    v.remove_invalid_points(a_body.transform, b_body.transform);

                    v.generation = self.generation;

                    assert!(self.contact_map.contains_key(&(b, IndexedRange::Exact(a))));
                }
            };
        }

        self.update_persistent_contacts();
    }

    fn update_persistent_contacts(&mut self) {
        self.islands
            .merge_root_islands(&mut self.contacts, &mut self.bodies);

        self.islands.verify_depth();

        self.islands.verify(&self.bodies, &self.contacts);

        let mut to_split = BTreeSet::new();
        // NOTE: removed bodies will not be found in contacts and are removed here as well
        let removed_contacts = self
            .contacts
            .iter()
            .filter(|v| v.1.generation != self.generation)
            .map(|v| v.0)
            .collect_vec();

        for contact in removed_contacts {
            to_split.insert(self.contacts[contact].island);

            tracing::info!(?contact, "unlinking");
            self.islands.unlink(&mut self.contacts, contact);

            let contact = self.contacts.remove(contact).unwrap();
            let a = contact.a.body;
            let b = contact.b.body;

            self.contact_map
                .remove(&(a, IndexedRange::Exact(b)))
                .unwrap();

            self.contact_map
                .remove(&(b, IndexedRange::Exact(a)))
                .unwrap();
        }

        self.islands.verify(&self.bodies, &self.contacts);

        for island in to_split {
            self.islands.verify(&self.bodies, &self.contacts);
            assert!(!self.islands.static_set().contains_key(island));

            self.islands.reconstruct(
                island,
                &mut self.bodies,
                &mut self.contacts,
                &self.contact_map,
            );

            self.islands.verify(&self.bodies, &self.contacts);
        }

        self.islands.verify(&self.bodies, &self.contacts);

        assert!(self.islands.to_remove().is_empty());

        for index in self.removed_bodies.drain() {
            self.bodies.remove(index);
            self.solver.remove_body(index);
        }
    }

    pub fn solve_contacts(&mut self) {
        assert!(self.removed_bodies.is_empty());
        for (_, contact) in &self.contacts {
            self.solver.apply_warmstart(contact);
        }

        for (_, contact) in &mut self.contacts {
            self.solver.solve_contact(contact).unwrap();
            self.dirty_bodies.extend([contact.a.body, contact.b.body]);
        }
    }

    pub fn sync_simulation_bodies(
        &mut self,
        query: &mut QueryBorrow<(Mutable<Vec3>, Mutable<Vec3>)>,
    ) {
        for body_index in self.bodies.keys() {
            let body = &self.solver.bodies()[body_index];
            let (vel, ang_vel) = query.get(body.id).expect("simulation body ");

            *vel = body.vel;
            *ang_vel = body.ang_vel;
        }

        self.dirty_bodies.clear();
    }

    pub(crate) fn remove_bodies(&mut self, removed: impl Iterator<Item = (Entity, BodyIndex)>) {
        for (_, index) in removed {
            self.islands.mark_for_remove(index);
            self.removed_bodies.insert(index);
        }
    }
}

#[derive(Fetch)]
struct ObjectQuery {
    pub transform: Component<Mat4>,
    pub mass: Opt<Component<f32>>,
    pub collider: Component<Collider>,
    pub offset: OptOr<Component<Mat4>, Mat4>,
    pub is_static: Satisfied<Component<()>>,
    pub is_trigger: Satisfied<Component<()>>,
    pub velocity: Component<Vec3>,
    pub angular_velocity: Component<Vec3>,
}

impl ObjectQuery {
    fn new() -> Self {
        Self {
            transform: world_transform(),
            mass: mass().opt(),
            collider: collider(),
            offset: collider_offset().opt_or_default(),
            is_static: is_static().satisfied(),
            velocity: velocity(),
            angular_velocity: angular_velocity(),
            is_trigger: is_trigger().satisfied(),
        }
    }
}

impl Default for ObjectQuery {
    fn default() -> Self {
        Self::new()
    }
}