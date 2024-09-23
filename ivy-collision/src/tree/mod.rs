use crate::body::{Body, BodyIndex};
use flax::{
    entity_ids, fetch::Satisfied, sink::Sink, CommandBuffer, Component, Entity, EntityIds, Error,
    Fetch, FetchExt, Opt, OptOr, World,
};
use glam::{Mat4, Vec3};
use ivy_core::{
    components::{angular_velocity, is_static, is_trigger, mass, velocity, world_transform},
    gizmos::{DrawGizmos, GizmosSection},
};
use slotmap::SlotMap;

use crate::{
    components::{body_index, collider, collider_offset},
    Collider,
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

pub struct CollisionTree {
    nodes: SlotMap<NodeIndex, BvhNode>,
    /// Objects removed from the tree due to splits. Bound to be replaced.
    /// Double buffer as insertions may cause new pops.
    root: NodeIndex,
    to_refit: Vec<BodyIndex>,
}

impl CollisionTree {
    pub fn new(root: BvhNode) -> Self {
        let mut nodes = SlotMap::with_key();

        let root = nodes.insert(root);

        Self {
            nodes,
            root,
            to_refit: Default::default(),
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

    pub fn insert_body(
        &mut self,
        bodies: &mut SlotMap<BodyIndex, Body>,
        index: BodyIndex,
    ) -> BodyIndex {
        // let index = self.body_data.insert_with_key(|index| {
        //     self.islands.create_island(index);
        //     body.island = index;
        //     if body.state.is_static() {
        //         self.islands.mark_static(index);
        //     }
        //     body
        // });

        // self.islands.add_body(&mut self.body_data, index);

        let body = &bodies[index];
        let root = &mut self.nodes[self.root];
        root.allocated_bounds = root.allocated_bounds.merge(body.extended_bounds);
        BvhNode::insert(self.root, &mut self.nodes, index, bodies);

        BvhNode::update_bounds(self.root, &mut self.nodes, bodies);

        index
    }

    /// Registers new entities in the tree
    pub fn register(&mut self, world: &World, cmd: &mut CommandBuffer) {
        // let mut query = Query::new((entity_ids(), ObjectQuery::new())).without(tree_index());

        // for (id, q) in query.borrow(world).iter() {
        //     let offset = *q.offset;
        //     let transform = *q.transform * offset;

        //     let bounds = q.collider.bounding_box(transform);
        //     let extended_bounds = bounds.expand(*q.velocity * 0.1);

        //     let body = Body {
        //         id: q.id,
        //         bounds,
        //         extended_bounds,
        //         transform,
        //         is_trigger: q.is_trigger,
        //         state: if q.is_static {
        //             NodeState::Static
        //         } else {
        //             NodeState::Dynamic
        //         },
        //         movable: q.mass.map(|v| v.is_normal()).unwrap_or(false),
        //         collider: q.collider.clone(),
        //         node: NodeIndex::null(),
        //         island: BodyIndex::null(),
        //         next_body: BodyIndex::null(),
        //         prev_body: BodyIndex::null(),
        //     };

        //     let tree_index = self.insert_body(id, body);

        //     BvhNode::update_bounds(self.root, &mut self.nodes, &self.body_data);
        //     cmd.set(id, components::tree_index(), tree_index);
        // }
    }

    pub fn update(&mut self, body_index: BodyIndex, body: &Body) -> Result<(), Error> {
        // let mut query = Query::new((tree_index(), ObjectQuery::new()));

        // Update object data
        // for (&object_index, q) in query.borrow(world).iter() {
        // let object_data = &mut self.body_data[object_index];
        // object_data.transform = *q.transform;
        // object_data.bounds = q.collider.bounding_box(*q.transform);
        // object_data.extended_bounds = object_data.bounds.expand(q.velocity.abs() * 0.1);

        let node = &self.nodes[body.node];

        if !node.allocated_bounds().contains(body.extended_bounds) {
            self.nodes[body.node]
                .remove(body_index)
                .expect("object not in node");

            self.to_refit.push(body_index);
        }
        // }

        Ok(())
    }

    pub fn refit(&mut self, bodies: &mut SlotMap<BodyIndex, Body>) {
        for &index in &self.to_refit {
            let root = &mut self.nodes[self.root];
            root.allocated_bounds = root.allocated_bounds.merge(bodies[index].extended_bounds);

            BvhNode::insert(self.root, &mut self.nodes, index, bodies)
        }

        self.to_refit.clear();

        BvhNode::update_bounds(self.root, &mut self.nodes, bodies);
        BvhNode::rebalance(self.root, &mut self.nodes, bodies);
    }

    pub fn check_collisions(
        &mut self,
        bodies: &SlotMap<BodyIndex, Body>,
        result: &mut Vec<(BodyIndex, BodyIndex)>,
    ) -> anyhow::Result<()> {
        BvhNode::check_collisions(self.root, &self.nodes, bodies, &mut |a, _, b, _| {
            result.push((a, b));
        });

        // self.islands.next_gen();

        // self.generation += 1;

        // for (mut a, mut a_obj, mut b, mut b_obj) in intersecting_pairs {
        //     // ensure stable indexing and links
        //     if a > b {
        //         std::mem::swap(&mut a, &mut b);
        //         std::mem::swap(&mut a_obj, &mut b_obj);
        //     }

        //     let Some(contact) = self.intersection_generator.intersect(
        //         &TransformedShape::new(&a_obj.collider, a_obj.transform),
        //         &TransformedShape::new(&b_obj.collider, b_obj.transform),
        //     ) else {
        //         continue;
        //     };

        //     match self.contact_map.entry((a, IndexedRange::Exact(b))) {
        //         Entry::Vacant(slot) => {
        //             // new island
        //             let contact = Contact {
        //                 a: EntityPayload {
        //                     entity: a_obj.id,
        //                     is_trigger: false,
        //                     state: a_obj.state,
        //                     body: a,
        //                 },
        //                 b: EntityPayload {
        //                     entity: b_obj.id,
        //                     is_trigger: false,
        //                     state: b_obj.state,
        //                     body: b,
        //                 },
        //                 surface: contact,
        //                 island: BodyIndex::null(),
        //                 next_contact: ContactIndex::null(),
        //                 prev_contact: ContactIndex::null(),
        //                 generation: self.generation,
        //             };

        //             let id = self.contacts.insert(contact);
        //             slot.insert(id);

        //             self.islands.link(&mut self.contacts, id);

        //             assert!(!self.contact_map.contains_key(&(b, IndexedRange::Exact(a))));

        //             self.contact_map.insert((b, IndexedRange::Exact(a)), id);
        //         }
        //         Entry::Occupied(v) => {
        //             let &contact_index = v.get();
        //             let v = &mut self.contacts[contact_index];
        //             v.surface = contact;
        //             v.generation = self.generation;
        //             assert!(self.contact_map.contains_key(&(b, IndexedRange::Exact(a))));
        //         }
        //     };
        // }

        // // self.islands.verify(&self.body_data, &self.contacts);
        // self.islands
        //     .merge_root_islands(&mut self.contacts, &mut self.body_data);
        // self.islands.verify_depth();

        // self.islands.verify(&self.body_data, &self.contacts);

        // let mut to_split = BTreeSet::new();
        // let removed_contacts = self
        //     .contacts
        //     .iter()
        //     .filter(|v| v.1.generation != self.generation)
        //     .map(|v| v.0)
        //     .collect_vec();

        // for contact in removed_contacts {
        //     to_split.insert(self.contacts[contact].island);

        //     tracing::info!(?contact, "unlinking");
        //     self.islands.unlink(&mut self.contacts, contact);

        //     let contact = self.contacts.remove(contact).unwrap();
        //     let a = contact.a.body;
        //     let b = contact.b.body;

        //     self.contact_map
        //         .remove(&(a, IndexedRange::Exact(b)))
        //         .unwrap();

        //     self.contact_map
        //         .remove(&(b, IndexedRange::Exact(a)))
        //         .unwrap();
        // }

        // self.islands.verify(&self.body_data, &self.contacts);

        // for island in to_split {
        //     self.islands.verify(&self.body_data, &self.contacts);
        //     // let rep = self
        //     //     .islands
        //     //     .representative_compress(island)
        //     //     .expect("Static bodies are never present as islands");

        //     assert!(!self.islands.static_set().contains_key(island));

        //     // assert_eq!(rep, island, "bodies shall only be stored in root islands");
        //     self.islands.reconstruct(
        //         island,
        //         &mut self.body_data,
        //         &mut self.contacts,
        //         &self.contact_map,
        //     );
        //     self.islands.verify(&self.body_data, &self.contacts);
        // }
        // self.islands.verify(&self.body_data, &self.contacts);

        Ok(())
    }

    /// Queries the tree with a given visitor. Traverses only the nodes that the
    /// visitor accepts and returns an iterator for each node containing the
    /// output of the visited node. Oftentimes, the output of the visitor is an
    /// iterator, which means that a nested iterator can be returned.
    pub fn query<V>(&self, visitor: V) -> TreeQuery<V> {
        TreeQuery::new(visitor, self, self.root)
    }
}

impl std::fmt::Debug for CollisionTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollisionTree")
            .field("root", &DebugNode::new(self.root, &self.nodes))
            .finish()
    }
}

// pub fn register_system() -> BoxedSystem {
//     System::builder()
//         .with_world()
//         .with_cmd_mut()
//         .with_query(Query::new(collision_tree().as_mut()))
//         .build(
//             |world: &World,
//              cmd: &mut CommandBuffer,
//              mut query: QueryBorrow<Mutable<CollisionTree>>| {
//                 query.iter().for_each(|tree| {
//                     tree.register(world, &mut *cmd);
//                 })
//             },
//         )
//         .boxed()
// }

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

// pub fn update_system() -> BoxedSystem {
//     System::builder()
//         .with_world()
//         .with_query(Query::new(collision_tree().as_mut()))
//         .build(
//             |world: &World, mut query: QueryBorrow<Mutable<CollisionTree>>| {
//                 query.iter().try_for_each(|tree| {
//                     tree.update(world)?;
//                     anyhow::Ok(())
//                 })
//             },
//         )
//         .boxed()
// }

// pub fn check_collisions_system() -> BoxedSystem {
//     System::builder()
//         .with_world()
//         .with_query(Query::new(collision_tree().as_mut()))
//         .build(
//             |world: &World, mut query: QueryBorrow<Mutable<CollisionTree>>| {
//                 query
//                     .iter()
//                     .try_for_each(|tree| tree.check_collisions(world))
//             },
//         )
//         .boxed()
// }

// pub fn collisions_tree_gizmos_system() -> BoxedSystem {
//     System::builder()
//         .with_query(Query::new(ivy_core::components::gizmos()))
//         .with_query(Query::new(collision_tree()))
//         .build(
//             |mut gizmos_query: QueryBorrow<Component<Gizmos>>,
//              mut query: QueryBorrow<Component<CollisionTree>>| {
//                 let mut section = gizmos_query
//                     .first()
//                     .unwrap()
//                     .begin_section("collisions_tree_gizmos_system");

//                 query.iter().for_each(|tree| section.draw(tree))
//             },
//         )
//         .boxed()
// }

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
        todo!()
        // BvhNode::draw_gizmos_recursive(
        //     self.root,
        //     &self.nodes,
        //     gizmos,
        //     &self.body_data,
        //     &mut HashSet::new(),
        //     0,
        // );
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
        arch.has(body_index().key())
    }

    fn matches_component(&self, desc: flax::component::ComponentDesc) -> bool {
        desc.key() == body_index().key()
    }
}
