use std::{any, mem};

use flume::{Receiver, Sender};
use hecs::{Entity, Satisfies};
use hecs_schedule::{CommandBuffer, GenericWorld, SubWorld, Write};
use ivy_base::{
    Color, DrawGizmos, Events, Gizmos, Position, Rotation, Scale, Static, TransformMatrix,
    TransformQuery, Trigger,
};
use ivy_resources::{DefaultResource, DefaultResourceMut};
use slotmap::SlotMap;
use smallvec::{Array, SmallVec};

use crate::{util::TOLERANCE, Collider, Sphere};

mod binary_node;
mod index;
mod intersect_visitor;
pub mod query;
mod traits;
mod visitor;

pub use binary_node::*;
pub use index::*;
pub use intersect_visitor::*;
pub use traits::*;
pub use visitor::*;

use self::query::TreeQuery;

pub type Nodes<N> = SlotMap<NodeIndex<N>, N>;

/// Marker for where the object is in the tree
#[derive(Debug, Clone)]
pub struct TreeMarker<N> {
    index: NodeIndex<N>,
    object: CollisionObject,
    on_drop: Sender<(NodeIndex<N>, Entity)>,
}

impl<N> Drop for TreeMarker<N> {
    fn drop(&mut self) {
        if !self.on_drop.is_disconnected() {
            self.on_drop
                .send((self.index, self.object.entity))
                .expect("Failed to send drop message")
        }
    }
}

pub struct CollisionTree<N> {
    nodes: SlotMap<NodeIndex<N>, N>,
    /// Objects removed from the tree due to splits. Bound to be replaced.
    /// Double buffer as insertions may cause new pops.
    popped: (Vec<CollisionObject>, Vec<CollisionObject>),
    iteration: usize,
    root: NodeIndex<N>,

    tx: Sender<(NodeIndex<N>, Entity)>,
    rx: Receiver<(NodeIndex<N>, Entity)>,
}

impl<N: 'static + CollisionTreeNode> CollisionTree<N> {
    pub fn new(root: N) -> Self {
        let mut nodes = SlotMap::with_key();

        let root = nodes.insert(root);
        let (tx, rx) = flume::unbounded();

        Self {
            nodes,
            popped: (Vec::new(), Vec::new()),
            root,
            iteration: 0,
            tx,
            rx,
        }
    }

    pub fn contains(&self, object: &CollisionObject) -> bool {
        self.nodes[self.root].contains(object)
    }

    /// Get a reference to the collision tree's nodes.
    pub fn nodes(&self) -> &SlotMap<NodeIndex<N>, N> {
        &self.nodes
    }

    /// Get a mutable reference to the collision tree's nodes.
    pub fn nodes_mut(&mut self) -> &mut SlotMap<NodeIndex<N>, N> {
        &mut self.nodes
    }

    pub fn handle_removed(&mut self) {
        let nodes = &mut self.nodes;
        self.rx.try_iter().for_each(|(index, e)| {
            let _ = nodes[index].remove(e);
        })
    }

    pub fn register(
        &mut self,
        world: SubWorld<(
            TransformQuery,
            &Collider,
            Satisfies<&Trigger>,
            Satisfies<&Static>,
        )>,
        cmd: &mut CommandBuffer,
    ) {
        let inserted = world
            .native_query()
            .without::<TreeMarker<N>>()
            .iter()
            .map(|(e, (transform, collider, is_trigger, is_static))| {
                CollisionObject::new(
                    e,
                    Sphere::enclose(collider, *transform.scale),
                    transform.into_matrix(),
                    is_trigger,
                    is_static,
                )
            })
            .collect::<Vec<_>>();

        inserted.into_iter().for_each(|object| {
            let entity = object.entity;
            let index = self
                .root
                .insert(&mut self.nodes, object, &mut self.popped.0);
            let marker = TreeMarker {
                index,
                object,
                on_drop: self.tx.clone(),
            };
            cmd.insert_one(entity, marker);
        })
    }

    pub fn update(
        &mut self,
        world: SubWorld<(
            TransformQuery,
            &Collider,
            &mut TreeMarker<N>,
            Satisfies<&Trigger>,
            Satisfies<&Static>,
        )>,
    ) -> Result<(), hecs_schedule::Error> {
        self.handle_removed();
        self.handle_popped(&world)?;

        self.iteration += 1;

        let nodes = &mut self.nodes;
        let iteration = self.iteration;

        world
            .query::<(&Scale, &Position, &Rotation, &Collider, &mut TreeMarker<N>)>()
            .iter()
            .for_each(|(_, (scale, pos, rot, collider, marker))| {
                let index = marker.index;

                // Bounds have changed
                if (marker.object.max_scale - scale.component_max()).abs() < TOLERANCE {
                    marker.object.bound = Sphere::enclose(collider, *scale)
                }

                marker.object.origin = *pos;
                marker.object.transform = TransformMatrix::new(*pos, *rot, *scale);

                nodes[index].set(marker.object, iteration)
            });

        let popped = &mut self.popped.0;

        // Move entities between nodes when they no longer fit or fit into a
        // deeper child.
        world
            .query::<&mut TreeMarker<N>>()
            .iter()
            .for_each(|(_, marker)| {
                let index = marker.index;
                let node = &nodes[index];

                let object = &marker.object;
                if !node.contains(object) || index.fits_child(nodes, object).is_some() {
                    nodes[index].remove(object.entity);
                    popped.push(marker.object)
                }
            });

        self.handle_popped(&world)?;

        Ok(())
    }

    pub fn handle_popped(&mut self, world: &impl GenericWorld) -> hecs_schedule::error::Result<()> {
        let nodes = &mut self.nodes;
        let root = self.root;
        while !self.popped.0.is_empty() {
            let (front, back) = &mut self.popped;

            front.drain(..).try_for_each(|obj| -> Result<_, _> {
                let mut marker = world.try_get_mut::<TreeMarker<N>>(obj.entity)?;

                marker.index = root.insert(nodes, obj, back);

                Ok(())
            })?;

            // Swap buffers and keep going
            mem::swap(&mut self.popped.0, &mut self.popped.1);
        }

        Ok(())
    }

    #[inline]
    pub fn check_collisions<'a, G: Array<Item = &'a CollisionObject>>(
        &'a self,
        world: SubWorld<&Collider>,
        events: &mut Events,
    ) -> hecs_schedule::error::Result<()> {
        let mut stack = SmallVec::<G>::new();

        self.root
            .check_collisions(&world, events, &self.nodes, &mut stack)
    }

    /// Queries the tree with a given visitor. Traverses only the nodes that the
    /// visitor accepts and returns an iterator for each node containing the
    /// output of the visited node. Oftentimes, the output of the visitor is an
    /// iterator, which means that a nested iterator can be returned.
    pub fn query<V>(&self, visitor: V) -> TreeQuery<N, V> {
        TreeQuery::new(visitor, &self.nodes, self.root)
    }
}

impl<N: CollisionTreeNode> std::fmt::Debug for CollisionTree<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollisionTree")
            .field("root", &DebugNode::new(self.root, &self.nodes))
            .finish()
    }
}

impl<N: CollisionTreeNode> CollisionTree<N> {
    pub fn register_system(
        world: SubWorld<(
            TransformQuery,
            &Collider,
            Satisfies<&Trigger>,
            Satisfies<&Static>,
        )>,
        mut cmd: Write<CommandBuffer>,
        mut tree: DefaultResourceMut<Self>,
    ) -> hecs_schedule::error::Result<()> {
        tree.register(world, &mut cmd);

        Ok(())
    }

    pub fn update_system(
        world: SubWorld<(
            TransformQuery,
            &Collider,
            &mut TreeMarker<N>,
            Satisfies<&Trigger>,
            Satisfies<&Static>,
        )>,
        mut tree: DefaultResourceMut<Self>,
    ) -> hecs_schedule::error::Result<()>
    where
        N: CollisionTreeNode,
    {
        tree.update(world)
    }

    pub fn check_collisions_system(
        world: SubWorld<&Collider>,
        tree: DefaultResourceMut<Self>,
        mut events: Write<Events>,
    ) -> hecs_schedule::error::Result<()>
    where
        N: CollisionTreeNode,
    {
        tree.check_collisions::<[&CollisionObject; 128]>(world, &mut events)?;

        Ok(())
    }
}

impl<N: CollisionTreeNode + DrawGizmos> CollisionTree<N> {
    pub fn draw_system(tree: DefaultResource<Self>, gizmos: DefaultResourceMut<Gizmos>) {
        tree.draw_gizmos(gizmos, Color::white())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CollisionObject {
    pub entity: Entity,
    pub bound: Sphere,
    pub origin: Position,
    pub transform: TransformMatrix,
    pub max_scale: f32,
    pub is_trigger: bool,
    pub is_static: bool,
}

/// Entity with additional contextual data
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EntityPayload {
    pub entity: Entity,
    pub is_trigger: bool,
    pub is_static: bool,
}

impl From<&CollisionObject> for EntityPayload {
    fn from(val: &CollisionObject) -> Self {
        Self {
            entity: val.entity,
            is_trigger: val.is_trigger,
            is_static: val.is_static,
        }
    }
}

impl std::ops::Deref for EntityPayload {
    type Target = Entity;

    fn deref(&self) -> &Self::Target {
        &self.entity
    }
}

impl CollisionObject {
    pub fn new(
        entity: Entity,
        bound: Sphere,
        transform: TransformMatrix,
        is_trigger: bool,
        is_static: bool,
    ) -> Self {
        Self {
            entity,
            bound,
            transform,
            origin: transform.extract_translation().into(),
            max_scale: transform[0][0].max(transform[1][1]).max(transform[2][2]),
            is_trigger,
            is_static,
        }
    }

    /// Get a reference to the object's entity.
    pub fn entity(&self) -> Entity {
        self.entity
    }

    //// Returns true if the bounding objects of the objects overlap
    fn overlaps(&self, other: &CollisionObject) -> bool {
        self.bound.overlaps(self.origin, &other.bound, other.origin)
    }
}

impl<N: CollisionTreeNode + DrawGizmos> DrawGizmos for CollisionTree<N> {
    fn draw_gizmos<T: std::ops::DerefMut<Target = Gizmos>>(
        &self,
        mut gizmos: T,
        color: ivy_base::Color,
    ) {
        gizmos.begin_section(any::type_name::<Self>());
        self.root
            .draw_gizmos_recursive(&self.nodes, &mut gizmos, color)
    }
}
