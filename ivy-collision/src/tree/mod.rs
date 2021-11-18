use std::{any, mem};

use hecs::{Entity, World};
use ivy_base::{
    DrawGizmos, Events, Gizmos, Position, Rotation, Scale, TimedScope, TransformMatrix,
};
use slotmap::SlotMap;
use smallvec::{Array, SmallVec};
use ultraviolet::Vec3;

use crate::{util::TOLERANCE, Collider, Sphere};

mod binary_node;
mod index;
pub mod query;
mod traits;
mod visitor;

pub use binary_node::*;
pub use index::*;
pub use traits::*;
pub use visitor::*;

use self::query::TreeQuery;

pub type Nodes<N> = SlotMap<NodeIndex<N>, N>;

/// Marker for where the object is in the tree
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeMarker<N> {
    index: NodeIndex<N>,
    object: Object,
}

pub struct CollisionTree<N> {
    nodes: SlotMap<NodeIndex<N>, N>,
    /// Objects removed from the tree due to splits. Bound to be replaced.
    /// Double buffer as insertions may cause new pops.
    popped: (Vec<Object>, Vec<Object>),
    iteration: usize,
    root: NodeIndex<N>,
}

impl<N: 'static + Node> CollisionTree<N> {
    pub fn new(root: N) -> Self {
        let mut nodes = SlotMap::with_key();

        let root = nodes.insert(root);
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
    pub fn nodes(&self) -> &SlotMap<NodeIndex<N>, N> {
        &self.nodes
    }

    /// Get a mutable reference to the collision tree's nodes.
    pub fn nodes_mut(&mut self) -> &mut SlotMap<NodeIndex<N>, N> {
        &mut self.nodes
    }

    pub fn register(&mut self, world: &mut World) {
        let inserted = world
            .query::<(&Collider, &Position, &Rotation, &Scale)>()
            .without::<TreeMarker<N>>()
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
        // let _scope = TimedScope::new(|elapsed| eprintln!("Tree updating took {:.3?}", elapsed));

        self.register(world);

        self.handle_popped(world)?;

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

                marker.object.origin = **pos;
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
                    let mut marker = world.get_mut::<TreeMarker<N>>(obj.entity)?;

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
        // let _scope =
        //     TimedScope::new(|elapsed| eprintln!("Tree collision checking took {:.3?}", elapsed));
        let mut stack = SmallVec::<G>::new();

        self.root
            .check_collisions(world, events, &self.nodes, &mut stack)
    }

    /// Queries the tree with a given visitor. Traverses only the nodes that the
    /// visitor accepts and returns an iterator for each node containing the
    /// output of the visited node. Oftentimes, the output of the visitor is an
    /// iterator, which means that a nested iterator can be returned.
    pub fn query<V>(&self, visitor: V) -> TreeQuery<N, V> {
        TreeQuery::new(visitor, &self.nodes, self.root)
    }
}

impl<N: Node> std::fmt::Debug for CollisionTree<N> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CollisionTree")
            .field("root", &DebugNode::new(self.root, &self.nodes))
            .finish()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Object {
    pub entity: Entity,
    pub bound: Sphere,
    pub origin: Vec3,
    pub transform: TransformMatrix,
    pub max_scale: f32,
}

impl Object {
    pub fn new(entity: Entity, bound: Sphere, transform: TransformMatrix) -> Self {
        Self {
            entity,
            bound,
            transform,
            origin: transform.extract_translation(),
            max_scale: transform[0][0].max(transform[1][1]).max(transform[2][2]),
        }
    }

    /// Get a reference to the object's entity.
    pub fn entity(&self) -> Entity {
        self.entity
    }
}

impl<N: Node + DrawGizmos> DrawGizmos for CollisionTree<N> {
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
