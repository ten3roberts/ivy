use flax::{
    components::is_static, entity_ids, fetch::Satisfied, sink::Sink, BoxedSystem, CommandBuffer,
    Component, Entity, EntityIds, Error, Fetch, FetchExt, Mutable, Opt, OptOr, Query, QueryBorrow,
    System, World,
};
use glam::{Mat4, Vec3};
use ivy_core::{
    gizmos, is_trigger, mass, velocity, world_transform, DrawGizmos, Gizmos, GizmosSection,
};
use slotmap::{new_key_type, SlotMap};
use smallvec::Array;

use crate::{
    components::{self, collider, collider_offset, collision_tree, tree_index},
    BoundingBox, Collider, Collision, CollisionPrimitive,
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

new_key_type! {
    pub struct ObjectIndex;
}

pub struct CollisionTree {
    nodes: SlotMap<NodeIndex, BvhNode>,
    /// Objects removed from the tree due to splits. Bound to be replaced.
    /// Double buffer as insertions may cause new pops.
    root: NodeIndex,

    object_data: SlotMap<ObjectIndex, ObjectData>,
    active_collisions: Vec<Collision>,
}

impl CollisionTree {
    pub fn new(root: BvhNode) -> Self {
        let mut nodes = SlotMap::with_key();

        let root = nodes.insert(root);

        Self {
            nodes,
            root,
            object_data: SlotMap::with_key(),
            active_collisions: Vec::new(),
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

    fn insert_impl(&mut self, _: Entity, object: ObjectData) -> ObjectIndex {
        let index = self.object_data.insert(object);

        BvhNode::insert(self.root, &mut self.nodes, index, &mut self.object_data);

        index
    }

    /// Registers new entities in the tree
    pub fn register(&mut self, world: &World, cmd: &mut CommandBuffer) {
        let mut query = Query::new((entity_ids(), ObjectQuery::new())).without(tree_index());

        for (id, q) in query.borrow(world).iter() {
            let obj: ObjectData = q.into_object_data();
            let tree_index = self.insert_impl(id, obj);
            tracing::info!(?id, ?tree_index, "registering entity into tree");

            cmd.set(id, components::tree_index(), tree_index);
        }
    }

    pub fn update(&mut self, world: &World) -> Result<(), Error> {
        let mut query = Query::new((tree_index(), ObjectQuery::new()));

        let mut to_refit = Vec::new();

        // Update object data
        for (&object_index, q) in query.borrow(world).iter() {
            let object_data = &mut self.object_data[object_index];
            object_data.transform = *q.transform;
            object_data.bounds = q.collider.bounding_box(*q.transform);
            object_data.extended_bounds = object_data.bounds.expand(q.velocity.abs() * 0.1);

            if !object_data
                .containing_bounds
                .contains(object_data.extended_bounds)
            {
                self.nodes[object_data.node].remove(object_index);
                to_refit.push(object_index);
            }
        }

        for object in to_refit {
            BvhNode::insert(self.root, &mut self.nodes, object, &mut self.object_data)
        }

        Ok(())
    }

    pub fn check_collisions(&mut self, _: &World) -> anyhow::Result<()> {
        self.active_collisions.clear();

        BvhNode::check_collisions(
            self.root,
            &self.nodes,
            &self.object_data,
            &mut |collision: Collision| {
                self.active_collisions.push(collision);
            },
        );

        Ok(())
    }

    /// Queries the tree with a given visitor. Traverses only the nodes that the
    /// visitor accepts and returns an iterator for each node containing the
    /// output of the visited node. Oftentimes, the output of the visitor is an
    /// iterator, which means that a nested iterator can be returned.
    pub fn query<V>(&self, visitor: V) -> TreeQuery<V> {
        TreeQuery::new(visitor, self, self.root)
    }

    /// Get a reference to the collision tree's objects.
    pub fn objects(&self) -> &SlotMap<ObjectIndex, ObjectData> {
        &self.object_data
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
        .with_query(Query::new(gizmos()))
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
pub struct ObjectData {
    pub id: Entity,
    pub collider: Collider,
    pub bounds: BoundingBox,
    pub extended_bounds: BoundingBox,
    pub transform: Mat4,
    pub is_trigger: bool,
    pub state: NodeState,
    pub movable: bool,
    pub containing_bounds: BoundingBox,
    pub node: NodeIndex,
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

impl ObjectData {
    pub fn is_movable(&self) -> bool {
        self.state != NodeState::Static && self.movable
    }
}

impl ObjectQueryItem<'_> {
    fn into_object_data(self) -> ObjectData {
        let offset = *self.offset;
        let transform = *self.transform * offset;

        let bounds = self.collider.bounding_box(transform);
        let extended_bounds = bounds.expand(*self.velocity * 0.1);

        ObjectData {
            id: self.id,
            bounds,
            extended_bounds,
            transform,
            is_trigger: self.is_trigger,
            state: NodeState::Dynamic,
            // state: if self.is_sleeping.is_some() {
            //     NodeState::Sleeping
            // } else if self.is_static.is_some() {
            //     NodeState::Static
            // } else {
            //     NodeState::Dynamic
            // },
            movable: self.mass.map(|v| v.is_normal()).unwrap_or(false),
            collider: self.collider.clone(),
            containing_bounds: Default::default(),
            node: NodeIndex::null(),
        }
    }
}

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
}

/// Entity with additional contextual data
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityPayload {
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
        // self.root
        //     .draw_gizmos_recursive(&self.nodes, gizmos, &self.object_data);

        for collision in &self.active_collisions {
            collision.contact.draw_primitives(gizmos);
        }
    }
}

pub struct DespawnedSubscriber {
    tx: flume::Sender<ObjectIndex>,
}

impl DespawnedSubscriber {
    pub fn new(tx: flume::Sender<ObjectIndex>) -> Self {
        Self { tx }
    }
}

impl flax::events::EventSubscriber for DespawnedSubscriber {
    fn on_added(&self, _: &flax::archetype::Storage, _: &flax::events::EventData) {}

    fn on_modified(&self, _: &flax::events::EventData) {}

    fn on_removed(&self, storage: &flax::archetype::Storage, event: &flax::events::EventData) {
        let values = storage.downcast_ref::<ObjectIndex>();
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
