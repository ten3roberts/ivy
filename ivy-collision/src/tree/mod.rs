use std::mem;

use hecs::World;
use ivy_base::{Events, Gizmos, Position, Rotation, Scale, TimedScope, TransformMatrix};
use ivy_resources::Key;
use slotmap::SlotMap;
use smallvec::{Array, SmallVec};

use crate::{util::TOLERANCE, Collider, Cube, Sphere};

mod index;
mod node;
pub mod query;
mod visitor;

pub use index::*;
pub use node::*;
pub use visitor::*;

use self::query::TreeQuery;

/// Marker for where the object is in the tree
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeMarker {
    index: NodeIndex,
    object: Object,
}

pub(crate) type Nodes<T> = SlotMap<NodeIndex, Node<T>>;

pub struct CollisionTree<T: Array<Item = Object>> {
    nodes: SlotMap<NodeIndex, Node<T>>,
    /// Objects removed from the tree due to splits. Bound to be replaced.
    /// Double buffer as insertions may cause new pops.
    popped: (Vec<Object>, Vec<Object>),
    iteration: usize,
    root: NodeIndex,
}

impl<T: Array<Item = Object>> CollisionTree<T> {
    pub fn new(origin: Position, bounds: Cube) -> Self {
        let mut nodes = SlotMap::with_key();
        let root = nodes.insert(Node::new(NodeIndex::null(), 0, origin, bounds));
        Self {
            nodes,
            popped: (Vec::new(), Vec::new()),
            root,
            iteration: 0,
        }
    }

    pub fn contains(&self, object: &Object) -> bool {
        self.nodes[self.root].contains(object)
    }

    /// Get a reference to the collision tree's nodes.
    pub fn nodes(&self) -> &SlotMap<NodeIndex, Node<T>> {
        &self.nodes
    }

    /// Get a mutable reference to the collision tree's nodes.
    pub fn nodes_mut(&mut self) -> &mut SlotMap<NodeIndex, Node<T>> {
        &mut self.nodes
    }

    pub fn register(&mut self, world: &mut World) {
        let inserted = world
            .query::<(&Collider, &Position, &Rotation, &Scale)>()
            .without::<TreeMarker>()
            .iter()
            .map(|(e, (collider, position, rot, scale))| {
                Object::new(
                    e,
                    Sphere::enclose(collider, *scale),
                    TransformMatrix::new(*position, *rot, *scale),
                )
            })
            .collect::<Vec<_>>();

        inserted.into_iter().for_each(|object| {
            let entity = object.entity;
            let marker = self
                .root
                .insert(&mut self.nodes, object, &mut self.popped.0);
            world.insert_one(entity, marker).unwrap();
        })
    }

    pub fn update(&mut self, world: &mut World) -> Result<(), hecs::ComponentError> {
        let _scope = TimedScope::new(|elapsed| eprintln!("Tree updating took {:.3?}", elapsed));

        self.register(world);

        self.handle_popped(world)?;

        self.iteration += 1;

        let nodes = &mut self.nodes;
        let iteration = self.iteration;

        world
            .query::<(&Scale, &Position, &Rotation, &Collider, &mut TreeMarker)>()
            .iter()
            .for_each(|(_, (scale, pos, rot, collider, marker))| {
                let index = marker.index;

                // Bounds have changed
                if (marker.object.max_scale - scale.component_max()).abs() < TOLERANCE {
                    marker.object.bound = Sphere::enclose(collider, *scale)
                }

                marker.object.origin = **pos;
                marker.object.transform = TransformMatrix::new(*pos, *rot, *scale);

                nodes[index].set(marker.object, iteration)
            });

        let popped = &mut self.popped.0;

        // Move entities between nodes when they no longer fit or fit into a
        // deeper child.
        world
            .query::<&mut TreeMarker>()
            .iter()
            .for_each(|(_, marker)| {
                let index = marker.index;
                let node = &nodes[index];

                let object = &marker.object;
                if !node.contains(object) || node.fits_child(nodes, object).is_some() {
                    nodes[index].remove(object.entity);
                    popped.push(marker.object)
                }
            });

        self.handle_popped(world)?;

        Ok(())
    }

    pub fn handle_popped(&mut self, world: &mut World) -> Result<(), hecs::ComponentError> {
        let nodes = &mut self.nodes;
        let root = self.root;
        while !self.popped.0.is_empty() {
            let (front, back) = &mut self.popped;

            front
                .drain(..)
                .try_for_each(|obj| -> Result<_, hecs::ComponentError> {
                    let mut marker = world.get_mut::<TreeMarker>(obj.entity)?;

                    let new_marker = root.insert(nodes, obj, back);

                    // assert_ne!(marker.index, new_marker.index);

                    *marker = new_marker;

                    Ok(())
                })?;

            // Swap buffers and keep going
            mem::swap(&mut self.popped.0, &mut self.popped.1);
        }

        Ok(())
    }

    #[inline]
    pub fn check_collisions<'a, G: Array<Item = &'a Object>>(
        &'a self,
        world: &mut World,
        events: &mut Events,
    ) -> Result<(), hecs::ComponentError> {
        let _scope =
            TimedScope::new(|elapsed| eprintln!("Tree collision checking took {:.3?}", elapsed));
        let mut stack = SmallVec::<G>::new();

        self.root
            .check_collisions(world, events, &self.nodes, &mut stack)
    }

    pub fn draw_gizmos(&self, world: &mut World, gizmos: &mut Gizmos) {
        gizmos.begin_section("CollisionTree");
        self.root.draw_gizmos(world, &self.nodes, 0, gizmos);
    }

    /// Queries the tree with a given visitor. Traverses only the nodes that the
    /// visitor accepts and returns an iterator for each node containing the
    /// output of the visited node. Oftentimes, the output of the visitor is an
    /// iterator, which means that a nested iterator can be returned.
    pub fn query<V>(&self, visitor: V) -> TreeQuery<T, V> {
        TreeQuery::new(visitor, &self.nodes, self.root)
    }
}

impl<T: Array<Item = Object>> std::fmt::Debug for CollisionTree<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollisionTree")
            .field("root", &DebugNode::new(self.root, &self.nodes))
            .finish()
    }
}
