use std::collections::{btree_map::Entry, BTreeMap, BTreeSet, HashSet};

use flax::{
    entity_ids, fetch::Satisfied, sink::Sink, BoxedSystem, CommandBuffer, Component, Entity,
    EntityIds, Error, Fetch, FetchExt, Mutable, Opt, OptOr, Query, QueryBorrow, System, World,
};
use glam::{Mat4, Vec3};
use itertools::Itertools;
use ivy_core::{
    components::{is_static, is_trigger, mass, velocity, world_transform},
    gizmos::{DrawGizmos, Gizmos, GizmosSection},
};
use slotmap::{new_key_type, Key, SecondaryMap, SlotMap};

use crate::{
    components::{self, collider, collider_offset, collision_tree, tree_index},
    BoundingBox, Collider, Contact, IntersectionGenerator, Shape, TransformedShape,
};

mod binary_node;
mod bvh;
mod index;
mod intersect_visitor;
pub mod query;
mod traits;
mod visitor;

pub use bvh::*;
pub use index::*;
pub use intersect_visitor::*;
pub use traits::*;
pub use visitor::*;

use self::query::TreeQuery;

pub type Nodes<N> = SlotMap<NodeIndex, N>;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum IndexedRange<T> {
    Min,
    Exact(T),
    Max,
}

impl<T> IndexedRange<T> {
    fn as_exact(&self) -> Option<&T> {
        if let Self::Exact(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

new_key_type! {
    pub struct BodyIndex;
    pub struct ContactIndex;
}

type ContactMap = SlotMap<ContactIndex, Contact>;
type BodyMap = SlotMap<BodyIndex, Body>;

pub struct ContactIter<'a> {
    contacts: &'a ContactMap,
    index: ContactIndex,
    head: ContactIndex,
}

impl<'a> Iterator for ContactIter<'a> {
    type Item = (ContactIndex, &'a Contact);

    fn next(&mut self) -> Option<Self::Item> {
        let index = self.index;
        if index.is_null() {
            return None;
        }

        let contact = &self.contacts[index];
        self.index = contact.next_contact;
        assert_ne!(self.index, self.head, "circular contact list");
        Some((index, contact))
    }
}

#[derive(Debug, Clone)]
pub struct Island {
    // parent or self
    parent: BodyIndex,
    head_body: BodyIndex,
    // used to rebuild island graph components during split
    head_contact: ContactIndex,
}

impl Island {
    fn add_contact(&mut self, contacts: &mut ContactMap, contact_index: ContactIndex) {
        let contact = &mut contacts[contact_index];
        // assert!(contact.island.is_null());
        assert!(
            contact.next_contact.is_null(),
            "contact is already connected"
        );

        contact.next_contact = self.head_contact;
        contact.prev_contact = ContactIndex::null();

        if !self.head_contact.is_null() {
            contacts[self.head_contact].prev_contact = contact_index;
        }

        self.head_contact = contact_index;
    }

    fn remove_contact(&mut self, contacts: &mut ContactMap, contact_index: ContactIndex) {
        let contact = &mut contacts[contact_index];

        if contact_index == self.head_contact {
            let next = contact.next_contact;
            contact.next_contact = ContactIndex::null();
            contact.island = BodyIndex::null();
            assert!(contact.prev_contact.is_null());

            self.head_contact = next;

            if !next.is_null() {
                let next = &mut contacts[next];
                assert_eq!(next.prev_contact, contact_index);
                next.prev_contact = ContactIndex::null();
            }
        } else {
            let prev = contact.prev_contact;
            assert!(!prev.is_null());
            let next = contact.next_contact;

            assert_eq!(contacts[prev].next_contact, contact_index);

            contacts[prev].next_contact = next;
            if !next.is_null() {
                contacts[next].prev_contact = prev;
            }
        }
    }

    fn add_body(&mut self, bodies: &mut BodyMap, body_index: BodyIndex) {
        let body = &mut bodies[body_index];
        assert!(body.next_body.is_null());

        body.next_body = self.head_body;

        if !self.head_body.is_null() {
            bodies[self.head_body].prev_body = body_index;
        }

        self.head_body = body_index;
        assert!(!self.head_body.is_null());
    }

    fn contacts<'a>(&self, contacts: &'a ContactMap) -> ContactIter<'a> {
        ContactIter {
            head: self.head_contact,
            contacts,
            index: self.head_contact,
        }
    }

    pub fn parent(&self) -> BodyIndex {
        self.parent
    }
}

#[derive(Default)]
struct Scratch {
    bodies: Vec<BodyIndex>,
    visited_bodies: SecondaryMap<BodyIndex, ()>,
    visited_contacts: SecondaryMap<ContactIndex, ()>,
}

pub(crate) struct Islands {
    islands: SecondaryMap<BodyIndex, Island>,
    static_set: SecondaryMap<BodyIndex, ()>,
    scratch: Scratch,
}

impl Islands {
    pub(crate) fn new() -> Self {
        Self {
            islands: Default::default(),
            static_set: Default::default(),
            scratch: Default::default(),
        }
    }

    fn create_island(&mut self, body_index: BodyIndex) {
        let island = Island {
            parent: body_index,
            head_body: BodyIndex::null(),
            head_contact: ContactIndex::null(),
        };

        self.islands.insert(body_index, island);
    }

    fn representative_compress(&mut self, index: BodyIndex) -> Option<BodyIndex> {
        let mut index = index;
        if self.static_set.contains_key(index) {
            return None;
        }

        loop {
            let node = &mut self.islands[index];
            let parent = node.parent;

            if parent == index {
                break;
            }

            let next_parent = self.islands[parent].parent;
            self.islands[index].parent = next_parent;

            index = parent;
        }

        Some(index)
    }

    fn link(&mut self, contacts: &mut ContactMap, contact_index: ContactIndex) -> &mut Island {
        let contact = &mut contacts[contact_index];
        let a = contact.a.body;
        let b = contact.b.body;

        let a_rep = self.representative_compress(a);
        let b_rep = self.representative_compress(b);

        match (a_rep, b_rep) {
            (None, None) => {
                panic!("static bodies can not collide");
            }
            (None, Some(b_rep)) => {
                contact.island = b_rep;
                let island = &mut self.islands[b_rep];
                island.add_contact(contacts, contact_index);
                island
            }
            (Some(a_rep), None) => {
                contact.island = a_rep;
                let island = &mut self.islands[a_rep];
                island.add_contact(contacts, contact_index);
                island
            }
            (Some(a_rep), Some(b_rep)) if a_rep == b_rep => {
                contact.island = a_rep;
                let island = &mut self.islands[a_rep];

                island.add_contact(contacts, contact_index);
                island
            }
            (Some(a_rep), Some(b_rep)) => {
                contact.island = a_rep;
                self.islands[b_rep].parent = a_rep;

                let island = &mut self.islands[a_rep];

                island.add_contact(contacts, contact_index);
                island
            }
        }
    }

    // Unlink a contact from it's island
    //
    // Does not split the island
    fn unlink(&mut self, contacts: &mut ContactMap, contact_index: ContactIndex) {
        let contact = &mut contacts[contact_index];

        let island_index = contact.island;
        contact.island = BodyIndex::null();
        let island = &mut self.islands[island_index];
        island.remove_contact(contacts, contact_index);
    }

    /// merges all island bodies into the roots
    fn merge_root_islands(&mut self, contacts: &mut ContactMap, bodies: &mut BodyMap) {
        let keys = self.islands.keys().collect_vec();
        for index in keys {
            let Some(parent_index) = self.representative_compress(index) else {
                continue;
            };

            if parent_index == index {
                continue;
            }

            let [island, parent] = self
                .islands
                .get_disjoint_mut([index, parent_index])
                .unwrap();

            let mut contact_index = island.head_contact;
            while !contact_index.is_null() {
                let contact = &mut contacts[contact_index];
                contact.island = parent_index;

                let next = contact.next_contact;
                // reached end, attach parent list
                if next.is_null() {
                    contact.next_contact = parent.head_contact;
                    if !parent.head_contact.is_null() {
                        assert!(contacts[parent.head_contact].prev_contact.is_null());
                        contacts[parent.head_contact].prev_contact = contact_index;
                    }

                    parent.head_contact = island.head_contact;
                }

                contact_index = next;
            }

            let mut body_index = island.head_body;
            while !body_index.is_null() {
                let body = &mut bodies[body_index];
                body.island = parent_index;

                let next = body.next_body;
                // reached end, attach parent list
                if next.is_null() {
                    body.next_body = parent.head_body;
                    if !parent.head_body.is_null() {
                        assert!(bodies[parent.head_body].prev_body.is_null());
                        bodies[parent.head_body].prev_body = body_index;
                    }

                    parent.head_body = island.head_body;
                    assert!(!island.head_body.is_null());
                }

                body_index = next;
            }

            island.head_body = BodyIndex::null();
            island.head_contact = ContactIndex::null();
        }
    }

    fn reconstruct(
        &mut self,
        island_index: BodyIndex,
        bodies: &mut BodyMap,
        contacts: &mut ContactMap,
        contact_map: &BTreeMap<(BodyIndex, IndexedRange<BodyIndex>), ContactIndex>,
    ) {
        let island = &self.islands[island_index];

        let mut body_index = island.head_body;

        let all_bodies = &mut self.scratch.bodies;
        all_bodies.clear();
        while !body_index.is_null() {
            let body = &bodies[body_index];
            all_bodies.push(body_index);
            body_index = body.next_body;
        }

        assert!(!all_bodies.is_empty());
        let visited = &mut self.scratch.visited_bodies;
        visited.clear();

        let visited_contacts = &mut self.scratch.visited_contacts;
        visited_contacts.clear();

        for &mut body_index in all_bodies {
            // found in connection from another seed
            if visited.contains_key(body_index) {
                continue;
            }

            let seed_index = body_index;

            let seed_island = &mut self.islands[seed_index];
            *seed_island = Island {
                parent: body_index,
                head_body: BodyIndex::null(),
                head_contact: ContactIndex::null(),
            };

            let mut stack = vec![body_index];
            while let Some(body_index) = stack.pop() {
                visited.insert(body_index, ());

                let body = &mut bodies[body_index];
                body.island = seed_index;
                body.next_body = BodyIndex::null();
                body.prev_body = BodyIndex::null();

                seed_island.add_body(bodies, body_index);

                let edges = contact_map
                    .range((body_index, IndexedRange::Min)..(body_index, IndexedRange::Max));

                for ((_, other_index), &contact_index) in edges {
                    let &other_index = other_index.as_exact().unwrap();

                    if visited_contacts.contains_key(contact_index) {
                        continue;
                    }

                    visited_contacts.insert(contact_index, ());

                    // connect contact to this island
                    let contact = &mut contacts[contact_index];

                    contact.island = seed_index;
                    contact.next_contact = ContactIndex::null();
                    contact.prev_contact = ContactIndex::null();

                    seed_island.add_contact(contacts, contact_index);

                    // do not link static bodies to island
                    if bodies[other_index].state.is_static() {
                        continue;
                    }

                    if !visited.contains_key(other_index) {
                        stack.push(other_index);
                    }
                }
            }
        }
    }

    fn add_body(&mut self, bodies: &mut SlotMap<BodyIndex, Body>, index: BodyIndex) {
        self.islands[index].add_body(bodies, index)
    }
}

pub struct CollisionTree {
    nodes: SlotMap<NodeIndex, BvhNode>,
    /// Objects removed from the tree due to splits. Bound to be replaced.
    /// Double buffer as insertions may cause new pops.
    root: NodeIndex,

    body_data: SlotMap<BodyIndex, Body>,
    intersection_generator: IntersectionGenerator,

    contacts: SlotMap<ContactIndex, Contact>,
    contact_map: BTreeMap<(BodyIndex, IndexedRange<BodyIndex>), ContactIndex>,

    islands: Islands,
    generation: u32,
}

impl CollisionTree {
    pub fn new(root: BvhNode) -> Self {
        let mut nodes = SlotMap::with_key();

        let root = nodes.insert(root);

        Self {
            nodes,
            root,
            body_data: SlotMap::with_key(),
            intersection_generator: Default::default(),
            islands: Islands::new(),
            contacts: Default::default(),
            contact_map: Default::default(),
            generation: 0,
        }
    }

    /// Get a reference to the collision tree's nodes.
    pub fn nodes(&self) -> &SlotMap<NodeIndex, BvhNode> {
        &self.nodes
    }

    pub fn node(&self, index: NodeIndex) -> Option<&BvhNode> {
        self.nodes.get(index)
    }

    /// Get a mutable reference to the collision tree's nodes.
    pub fn nodes_mut(&mut self) -> &mut SlotMap<NodeIndex, BvhNode> {
        &mut self.nodes
    }

    fn insert_body(&mut self, _: Entity, body: Body) -> BodyIndex {
        let index = self.body_data.insert_with_key(|index| {
            self.islands.create_island(index);
            if body.state.is_static() {
                self.islands.static_set.insert(index, ());
            }
            body
        });

        self.islands.add_body(&mut self.body_data, index);

        let root = &mut self.nodes[self.root];
        root.allocated_bounds = root
            .allocated_bounds
            .merge(self.body_data[index].extended_bounds);
        BvhNode::insert(self.root, &mut self.nodes, index, &mut self.body_data);

        index
    }

    /// Registers new entities in the tree
    pub fn register(&mut self, world: &World, cmd: &mut CommandBuffer) {
        let mut query = Query::new((entity_ids(), ObjectQuery::new())).without(tree_index());

        for (id, q) in query.borrow(world).iter() {
            let offset = *q.offset;
            let transform = *q.transform * offset;

            let bounds = q.collider.bounding_box(transform);
            let extended_bounds = bounds.expand(*q.velocity * 0.1);

            let body = Body {
                id: q.id,
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

            let tree_index = self.insert_body(id, body);

            BvhNode::update_bounds(self.root, &mut self.nodes, &self.body_data);
            cmd.set(id, components::tree_index(), tree_index);
        }
    }

    pub fn update(&mut self, world: &World) -> Result<(), Error> {
        let mut query = Query::new((tree_index(), ObjectQuery::new()));

        let mut to_refit = Vec::new();

        // Update object data
        for (&object_index, q) in query.borrow(world).iter() {
            let object_data = &mut self.body_data[object_index];
            object_data.transform = *q.transform;
            object_data.bounds = q.collider.bounding_box(*q.transform);
            object_data.extended_bounds = object_data.bounds.expand(q.velocity.abs() * 0.1);

            let node = &self.nodes[object_data.node];

            if !node
                .allocated_bounds()
                .contains(object_data.extended_bounds)
            {
                self.nodes[object_data.node]
                    .remove(object_index)
                    .expect("object not in node");

                to_refit.push(object_index);
            }
        }

        for object in to_refit {
            let root = &mut self.nodes[self.root];
            root.allocated_bounds = root
                .allocated_bounds
                .merge(self.body_data[object].extended_bounds);

            BvhNode::insert(self.root, &mut self.nodes, object, &mut self.body_data)
        }

        BvhNode::update_bounds(self.root, &mut self.nodes, &self.body_data);

        BvhNode::rebalance(self.root, &mut self.nodes, &mut self.body_data);

        Ok(())
    }

    pub fn check_collisions(&mut self, _: &World) -> anyhow::Result<()> {
        let mut intersecting_pairs = Vec::new();

        BvhNode::check_collisions(
            self.root,
            &self.nodes,
            &self.body_data,
            &mut |a, a_obj, b, b_obj| {
                intersecting_pairs.push((a, a_obj, b, b_obj));
            },
        );

        // self.islands.next_gen();

        self.generation += 1;

        for (mut a, mut a_obj, mut b, mut b_obj) in intersecting_pairs {
            // ensure stable indexing and links
            if a > b {
                std::mem::swap(&mut a, &mut b);
                std::mem::swap(&mut a_obj, &mut b_obj);
            }

            let Some(contact) = self.intersection_generator.intersect(
                &TransformedShape::new(&a_obj.collider, a_obj.transform),
                &TransformedShape::new(&b_obj.collider, b_obj.transform),
            ) else {
                continue;
            };

            match self.contact_map.entry((a, IndexedRange::Exact(b))) {
                Entry::Vacant(slot) => {
                    // new island
                    let contact = Contact {
                        a: EntityPayload {
                            entity: a_obj.id,
                            is_trigger: false,
                            state: a_obj.state,
                            body: a,
                        },
                        b: EntityPayload {
                            entity: b_obj.id,
                            is_trigger: false,
                            state: b_obj.state,
                            body: b,
                        },
                        surface: contact,
                        island: BodyIndex::null(),
                        next_contact: ContactIndex::null(),
                        prev_contact: ContactIndex::null(),
                        generation: self.generation,
                    };

                    let id = self.contacts.insert(contact);
                    self.islands.link(&mut self.contacts, id);

                    slot.insert(id);
                    assert!(!self.contact_map.contains_key(&(b, IndexedRange::Exact(a))));

                    self.contact_map.insert((b, IndexedRange::Exact(a)), id);
                }
                Entry::Occupied(v) => {
                    let &contact_index = v.get();
                    let v = &mut self.contacts[contact_index];
                    v.surface = contact;
                    v.generation = self.generation;
                    assert!(self.contact_map.contains_key(&(b, IndexedRange::Exact(a))));
                }
            };
        }

        self.islands
            .merge_root_islands(&mut self.contacts, &mut self.body_data);

        let mut to_split = BTreeSet::new();
        let removed_contacts = self
            .contacts
            .iter()
            .filter(|v| v.1.generation != self.generation)
            .map(|v| v.0)
            .collect_vec();

        for contact in removed_contacts {
            to_split.insert(self.contacts[contact].island);
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

        for island in to_split {
            let rep = self
                .islands
                .representative_compress(island)
                .expect("Static bodies are never present as islands");

            assert_eq!(rep, island, "bodies shall only be stored in root islands");
            self.islands.reconstruct(
                rep,
                &mut self.body_data,
                &mut self.contacts,
                &self.contact_map,
            );
        }

        Ok(())
    }

    pub fn islands(&self) -> slotmap::secondary::Iter<BodyIndex, Island> {
        self.islands.islands.iter()
    }

    pub fn island_contacts(&self, island: &Island) -> ContactIter {
        island.contacts(&self.contacts)
    }

    pub fn contacts(&self) -> slotmap::basic::Iter<ContactIndex, Contact> {
        self.contacts.iter()
    }

    /// Queries the tree with a given visitor. Traverses only the nodes that the
    /// visitor accepts and returns an iterator for each node containing the
    /// output of the visited node. Oftentimes, the output of the visitor is an
    /// iterator, which means that a nested iterator can be returned.
    pub fn query<V>(&self, visitor: V) -> TreeQuery<V> {
        TreeQuery::new(visitor, self, self.root)
    }

    /// Get a reference to the collision tree's objects.
    pub fn objects(&self) -> &SlotMap<BodyIndex, Body> {
        &self.body_data
    }

    pub fn body(&self, body: BodyIndex) -> &Body {
        &self.body_data[body]
    }
}

impl std::fmt::Debug for CollisionTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollisionTree")
            .field("root", &DebugNode::new(self.root, &self.nodes))
            .finish()
    }
}

pub fn register_system() -> BoxedSystem {
    System::builder()
        .with_world()
        .with_cmd_mut()
        .with_query(Query::new(collision_tree().as_mut()))
        .build(
            |world: &World,
             cmd: &mut CommandBuffer,
             mut query: QueryBorrow<Mutable<CollisionTree>>| {
                query.iter().for_each(|tree| {
                    tree.register(world, &mut *cmd);
                })
            },
        )
        .boxed()
}

// pub fn draw_system<N: CollisionTreeNode>(state: Component<CollisionTree<N>>) -> BoxedSystem {
//     System::builder()
//         .with_world()
//         .with_cmd_mut()
//         .with_query(Query::new(state.as_mut()))
//         .build(
//             |world: &World,
//              cmd: &mut CommandBuffer,
//              mut query: QueryBorrow<Mutable<CollisionTree<N>>>| {
//                 query.iter().for_each(|tree| {
//                     tree.register(world, &mut *cmd);
//                 })
//             },
//         )
//         .boxed()
// }

pub fn update_system() -> BoxedSystem {
    System::builder()
        .with_world()
        .with_query(Query::new(collision_tree().as_mut()))
        .build(
            |world: &World, mut query: QueryBorrow<Mutable<CollisionTree>>| {
                query.iter().try_for_each(|tree| {
                    tree.update(world)?;
                    anyhow::Ok(())
                })
            },
        )
        .boxed()
}

pub fn check_collisions_system() -> BoxedSystem {
    System::builder()
        .with_world()
        .with_query(Query::new(collision_tree().as_mut()))
        .build(
            |world: &World, mut query: QueryBorrow<Mutable<CollisionTree>>| {
                query
                    .iter()
                    .try_for_each(|tree| tree.check_collisions(world))
            },
        )
        .boxed()
}

pub fn collisions_tree_gizmos_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new(ivy_core::components::gizmos()))
        .with_query(Query::new(collision_tree()))
        .build(
            |mut gizmos_query: QueryBorrow<Component<Gizmos>>,
             mut query: QueryBorrow<Component<CollisionTree>>| {
                let mut section = gizmos_query
                    .first()
                    .unwrap()
                    .begin_section("collisions_tree_gizmos_system");

                query.iter().for_each(|tree| section.draw(tree))
            },
        )
        .boxed()
}

#[derive(Debug, Clone)]
/// Data contained in each object in the tree.
///
/// Copied and retained from the ECS for easy access
/// TODO: reduce size
pub struct Body {
    pub id: Entity,
    pub collider: Collider,
    pub bounds: BoundingBox,
    pub extended_bounds: BoundingBox,
    pub transform: Mat4,
    pub is_trigger: bool,
    pub state: NodeState,
    pub movable: bool,
    pub node: NodeIndex,

    // island links
    pub island: BodyIndex,
    pub next_body: BodyIndex,
    pub prev_body: BodyIndex,
}

#[derive(Fetch)]
pub struct ObjectQuery {
    id: EntityIds,
    transform: Component<Mat4>,
    mass: Opt<Component<f32>>,
    collider: Component<Collider>,
    offset: OptOr<Component<Mat4>, Mat4>,
    is_static: Satisfied<Component<()>>,
    is_trigger: Satisfied<Component<()>>,
    velocity: Component<Vec3>,
}

impl ObjectQuery {
    fn new() -> Self {
        Self {
            id: entity_ids(),
            transform: world_transform(),
            mass: mass().opt(),
            collider: collider(),
            offset: collider_offset().opt_or_default(),
            is_static: is_static().satisfied(),
            velocity: velocity(),
            is_trigger: is_trigger().satisfied(),
        }
    }
}

impl Body {
    pub fn is_movable(&self) -> bool {
        self.state != NodeState::Static && self.movable
    }
}

// impl ObjectQueryItem<'_> {
//     fn into_object_data(self) -> Body {
//         let offset = *self.offset;
//         let transform = *self.transform * offset;

//         let bounds = self.collider.bounding_box(transform);
//         let extended_bounds = bounds.expand(*self.velocity * 0.1);

//         Body {
//             id: self.id,
//             bounds,
//             extended_bounds,
//             transform,
//             is_trigger: self.is_trigger,
//             state: if self.is_static {
//                 NodeState::Static
//             } else {
//                 NodeState::Dynamic
//             },
//             // state: if self.is_sleeping.is_some() {
//             //     NodeState::Sleeping
//             // } else if self.is_static.is_some() {
//             //     NodeState::Static
//             // } else {
//             //     NodeState::Dynamic
//             // },
//             movable: self.mass.map(|v| v.is_normal()).unwrap_or(false),
//             collider: self.collider.clone(),
//             containing_bounds: Default::default(),
//             node: NodeIndex::null(),
//         }
//     }
// }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeState {
    Dynamic,
    Static,
    Sleeping,
}

impl NodeState {
    pub fn merge(self, other: Self) -> Self {
        match (self, other) {
            (Self::Dynamic, _) => Self::Dynamic,
            (_, Self::Dynamic) => Self::Dynamic,
            (Self::Sleeping, _) => Self::Sleeping,
            (_, Self::Sleeping) => Self::Sleeping,
            (NodeState::Static, NodeState::Static) => NodeState::Static,
        }
    }

    /// Returns `true` if the node state is [`Static`].
    ///
    /// [`Static`]: NodeState::Static
    pub fn is_static(&self) -> bool {
        matches!(self, Self::Static)
    }

    #[inline(always)]
    pub fn dormant(&self) -> bool {
        *self == NodeState::Static || *self == NodeState::Sleeping
    }

    /// Returns `true` if the node state is [`Sleeping`].
    ///
    /// [`Sleeping`]: NodeState::Sleeping
    pub fn is_sleeping(&self) -> bool {
        matches!(self, Self::Sleeping)
    }

    /// Returns `true` if the node state is [`Dynamic`].
    ///
    /// [`Dynamic`]: NodeState::Dynamic
    pub fn is_dynamic(&self) -> bool {
        matches!(self, Self::Dynamic)
    }

    fn inflate_amount(&self) -> f32 {
        match self {
            NodeState::Dynamic => 0.0,
            NodeState::Static => 0.0,
            NodeState::Sleeping => 0.0,
        }
    }
}

/// Entity with additional contextual data
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityPayload {
    pub body: BodyIndex,
    pub entity: Entity,
    pub is_trigger: bool,
    pub state: NodeState,
}

impl std::ops::Deref for EntityPayload {
    type Target = Entity;

    fn deref(&self) -> &Self::Target {
        &self.entity
    }
}

impl DrawGizmos for CollisionTree {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        BvhNode::draw_gizmos_recursive(
            self.root,
            &self.nodes,
            gizmos,
            &self.body_data,
            &mut HashSet::new(),
            0,
        );
    }
}

pub struct DespawnedSubscriber {
    tx: flume::Sender<BodyIndex>,
}

impl DespawnedSubscriber {
    pub fn new(tx: flume::Sender<BodyIndex>) -> Self {
        Self { tx }
    }
}

impl flax::events::EventSubscriber for DespawnedSubscriber {
    fn on_added(&self, _: &flax::archetype::Storage, _: &flax::events::EventData) {}

    fn on_modified(&self, _: &flax::events::EventData) {}

    fn on_removed(&self, storage: &flax::archetype::Storage, event: &flax::events::EventData) {
        let values = storage.downcast_ref::<BodyIndex>();
        event.slots.iter().for_each(|slot| {
            self.tx.send(values[slot]).unwrap();
        });
    }

    fn is_connected(&self) -> bool {
        self.tx.is_connected()
    }

    fn matches_arch(&self, arch: &flax::archetype::Archetype) -> bool {
        arch.has(tree_index().key())
    }

    fn matches_component(&self, desc: flax::component::ComponentDesc) -> bool {
        desc.key() == tree_index().key()
    }
}
