use flax::{
    components::is_static, entity_ids, fetch::Satisfied, sink::Sink, BoxedSystem, CommandBuffer,
    Component, Entity, Error, Fetch, FetchExt, Mutable, Opt, OptOr, Query, QueryBorrow, System,
    World,
};
use flume::{Receiver, Sender};
use glam::{Mat4, Vec3};
use ivy_core::{is_trigger, mass, velocity, world_transform, DrawGizmos, Events, GizmosSection};
use slotmap::{new_key_type, SlotMap};
use smallvec::Array;

use crate::{
    components::{self, collider, collider_offset, tree_index},
    BoundingBox, Collider, CollisionPrimitive,
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

pub struct OnDrop {
    object: Object,
    tx: Sender<Object>,
}

impl Drop for OnDrop {
    fn drop(&mut self) {
        if !self.tx.is_disconnected() {
            self.tx.send(self.object).unwrap()
        }
    }
}

new_key_type! {
    pub struct ObjectIndex;
}

/// Marker for where the object is in the tree
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Object {
    pub entity: Entity,
    pub index: ObjectIndex,
}

pub struct CollisionTree<N> {
    nodes: SlotMap<NodeIndex, N>,
    /// Objects removed from the tree due to splits. Bound to be replaced.
    /// Double buffer as insertions may cause new pops.
    root: NodeIndex,

    objects: SlotMap<ObjectIndex, ObjectData>,
    /// Objects that need to be reinserted into the tree
    popped: Vec<Object>,
    rx: Receiver<ObjectIndex>,
}

impl<N: CollisionTreeNode> CollisionTree<N> {
    pub fn new(root: N, despawned: Receiver<ObjectIndex>) -> Self {
        let mut nodes = SlotMap::with_key();

        let root = nodes.insert(root);

        Self {
            nodes,
            root,
            objects: SlotMap::with_key(),
            popped: Vec::new(),
            rx: despawned,
        }
    }

    /// Get a reference to the collision tree's nodes.
    pub fn nodes(&self) -> &SlotMap<NodeIndex, N> {
        &self.nodes
    }

    pub fn node(&self, index: NodeIndex) -> Option<&N> {
        self.nodes.get(index)
    }

    /// Get a mutable reference to the collision tree's nodes.
    pub fn nodes_mut(&mut self) -> &mut SlotMap<NodeIndex, N> {
        &mut self.nodes
    }

    fn insert(&mut self, entity: Entity, object: ObjectData) -> Object {
        let index = self.objects.insert(object);
        let object = Object { entity, index };

        N::insert(self.root, &mut self.nodes, object, &self.objects);

        object
    }

    /// Registers new entities in the tree
    pub fn register(&mut self, world: &World, cmd: &mut CommandBuffer) {
        let mut query = Query::new((entity_ids(), ObjectQuery::new())).without(tree_index());

        for (id, q) in query.borrow(world).iter() {
            let obj: ObjectData = q.into_object_data();
            let tree_index = self.insert(id, obj);
            tracing::info!(?id, ?tree_index.index, "registering entity into tree");

            cmd.set(id, components::tree_index(), tree_index.index);
        }
    }

    fn handle_despawned(&mut self) -> usize {
        self.rx
            .try_iter()
            .map(|index| {
                self.objects.remove(index);
            })
            .count()
    }

    pub fn update(&mut self, world: &World) -> Result<(), Error> {
        let mut query = Query::new((tree_index(), ObjectQuery::new()));
        // Update object data
        for (&index, q) in query.borrow(world).iter() {
            tracing::info!(?index);
            let data: ObjectData = q.into_object_data();
            self.objects[index] = data;
        }

        let mut despawned = self.handle_despawned();

        // Update tree
        N::update(
            self.root,
            &mut self.nodes,
            &self.objects,
            &mut self.popped,
            &mut despawned,
        );

        assert_eq!(despawned, 0);

        for object in self.popped.drain(..) {
            N::insert(self.root, &mut self.nodes, object, &self.objects)
        }

        Ok(())
    }

    #[inline]
    /// Checks the tree for collisions and dispatches them as events
    pub fn check_collisions<'a, G: Array<Item = &'a ObjectData>>(
        &'a self,
        _: &World,
        events: &mut Events,
    ) -> anyhow::Result<()> {
        N::check_collisions(events, self.root, &self.nodes, &self.objects);

        Ok(())
    }

    /// Queries the tree with a given visitor. Traverses only the nodes that the
    /// visitor accepts and returns an iterator for each node containing the
    /// output of the visited node. Oftentimes, the output of the visitor is an
    /// iterator, which means that a nested iterator can be returned.
    pub fn query<V>(&self, visitor: V) -> TreeQuery<N, V> {
        TreeQuery::new(visitor, self, self.root)
    }

    /// Get a reference to the collision tree's objects.
    pub fn objects(&self) -> &SlotMap<ObjectIndex, ObjectData> {
        &self.objects
    }
}

impl<N: CollisionTreeNode> std::fmt::Debug for CollisionTree<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollisionTree")
            .field("root", &DebugNode::new(self.root, &self.nodes))
            .finish()
    }
}

pub fn register_system<N: CollisionTreeNode>(state: Component<CollisionTree<N>>) -> BoxedSystem {
    System::builder()
        .with_world()
        .with_cmd_mut()
        .with_query(Query::new(state.as_mut()))
        .build(
            |world: &World,
             cmd: &mut CommandBuffer,
             mut query: QueryBorrow<Mutable<CollisionTree<N>>>| {
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

pub fn update_system<N: CollisionTreeNode>(state: Component<CollisionTree<N>>) -> BoxedSystem {
    System::builder()
        .with_world()
        .with_query(Query::new(state.as_mut()))
        .build(
            |world: &World, mut query: QueryBorrow<Mutable<CollisionTree<N>>>| {
                query.iter().try_for_each(|tree| {
                    tree.update(world)?;
                    anyhow::Ok(())
                })
            },
        )
        .boxed()
}
pub fn check_collisions_system<N: CollisionTreeNode>(
    state: Component<CollisionTree<N>>,
) -> BoxedSystem {
    System::builder()
        .with_world()
        .with_input_mut()
        .with_query(Query::new(state.as_mut()))
        .build(
            |world: &World,
             events: &mut Events,
             mut query: QueryBorrow<Mutable<CollisionTree<N>>>| {
                query.iter().try_for_each(|tree| {
                    tree.check_collisions::<[&ObjectData; 128]>(world, &mut *events)
                })
            },
        )
        .boxed()
}

// impl<N: CollisionTreeNode> CollisionTree<N> {
//     pub fn register_system(
//         &mut self,
//         world: &World,
//         mut cmd: &mut CommandBuffer,
//     ) -> anyhow::Result<()> {
//         self.register(world, &mut cmd);

//         Ok(())
//     }

//     pub fn update_system(world: &World, mut tree: DefaultResourceMut<Self>) -> anyhow::Result<()> {
//         tree.update(world).context("Failed to update tree")
//     }

//     pub fn check_collisions_system(
//         world: &World,
//         tree: DefaultResourceMut<Self>,
//         mut events: &mut Events,
//     ) -> anyhow::Result<()>
//     where
//         N: CollisionTreeNode,
//     {
//         tree.check_collisions::<[&ObjectData; 128]>(world, &mut events)?;

//         Ok(())
//     }
// }

// impl<N: CollisionTreeNode + DrawGizmos> CollisionTree<N> {
//     pub fn draw_system(tree: DefaultResource<Self>, mut gizmos: DefaultResourceMut<Gizmos>) {
//         tree.draw_gizmos(&mut gizmos, Srgba::new(1.0, 1.0, 1.0, 1.0))
//     }
// }

#[derive(Debug, Clone)]
/// Data contained in each object in the tree.
///
/// Copied and retained from the ECS for easy access
/// TODO: reduce size
pub struct ObjectData {
    pub collider: Collider,
    pub bounds: BoundingBox,
    pub extended_bounds: BoundingBox,
    pub transform: Mat4,
    pub is_trigger: bool,
    pub state: NodeState,
    pub movable: bool,
}
#[derive(Fetch)]
pub struct ObjectQuery {
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

// #[derive(Query)]
// pub struct ObjectQuery<'a> {
//     transform: TransformQuery<'a>,
//     mass: Option<&'a Mass>,
//     offset: Option<&'a ColliderOffset>,
//     collider: &'a Collider,
//     is_trigger: Option<&'a Trigger>,
//     is_static: Option<&'a Static>,
//     is_visible: Option<&'a Visible>,
//     is_sleeping: Option<&'a Sleeping>,
//     velocity: Option<&'a Velocity>,
// }

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

impl<N: CollisionTreeNode + DrawGizmos> DrawGizmos for CollisionTree<N> {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        self.root.draw_gizmos_recursive(&self.nodes, gizmos)
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
