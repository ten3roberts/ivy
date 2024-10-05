use crate::body::{Body, BodyIndex};
use flax::{sink::Sink, Entity, Error};
use slotmap::SlotMap;

use crate::components::body_index;

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

    pub fn update(&mut self, body_index: BodyIndex, body: &Body) -> Result<(), Error> {
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

        Ok(())
    }

    pub fn root(&self) -> NodeIndex {
        self.root
    }
}

impl std::fmt::Debug for CollisionTree {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollisionTree")
            .field("root", &DebugNode::new(self.root, &self.nodes))
            .finish()
    }
}

impl Body {
    pub fn is_movable(&self) -> bool {
        self.state != NodeState::Static && self.movable
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
