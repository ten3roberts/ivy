use std::mem;

use hecs::World;
use ivy_core::{Color, Events, Gizmos, Position, Rotation, Scale, TransformMatrix};
use ivy_resources::Key;
use slotmap::SlotMap;
use smallvec::{Array, SmallVec};
use ultraviolet::Vec3;

use crate::{Collider, Sphere};

mod index;
mod node;

pub use index::*;
pub use node::*;

/// Marker for where the object is in the tree
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeMarker {
    index: NodeIndex,
    object: Object,
}

type Nodes<const C: usize> = SlotMap<NodeIndex, Node<C>>;

pub struct CollisionTree<const CAP: usize> {
    nodes: SlotMap<NodeIndex, Node<CAP>>,
    /// Objects removed from the tree due to splits. Bound to be replaced.
    /// Double buffer as insertions may cause new pops.
    popped: (Vec<Object>, Vec<Object>),
    iteration: usize,
    root: NodeIndex,
}

impl<const CAP: usize> CollisionTree<CAP> {
    pub fn new(origin: Vec3, half_extents: Vec3) -> Self {
        let mut nodes = SlotMap::with_key();
        let root = nodes.insert(Node::new(NodeIndex::null(), 0, origin, half_extents));
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
    pub fn nodes(&self) -> &SlotMap<NodeIndex, Node<CAP>> {
        &self.nodes
    }

    /// Get a mutable reference to the collision tree's nodes.
    pub fn nodes_mut(&mut self) -> &mut SlotMap<NodeIndex, Node<CAP>> {
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
        self.iteration += 1;

        let nodes = &mut self.nodes;
        let iteration = self.iteration;

        world
            .query::<(
                &Scale,
                &Position,
                &Rotation,
                &Collider,
                &mut TreeMarker,
                &mut Color,
            )>()
            .iter()
            .for_each(|(_, (scale, pos, rot, collider, marker, color))| {
                let index = marker.index;
                let node = &nodes[index];

                *color = Color::hsl(node.depth as f32 * 60.0, 1.0, 0.5);

                // Bounds have changed
                if marker.object.max_scale != scale.component_max() {
                    marker.object.bound = Sphere::enclose(collider, *scale)
                }

                marker.object.origin = **pos;
                marker.object.transform = TransformMatrix::new(*pos, *rot, *scale);

                nodes[index].set(marker.object, iteration)
            });

        let popped = &mut self.popped.0;
        let root = self.root;

        // Mmve entities between nodes when they no longer fit or fit into a
        // deeper child.
        // world
        //     .query::<&mut TreeMarker>()
        //     .iter()
        //     .for_each(|(e, marker)| {
        //         let index = marker.index;
        //         let node = &nodes[index];

        //         let object = &marker.object;
        //         if !node.contains(object) {
        //             // eprintln!("No longer fits");
        //             index.remove(nodes, object.entity);
        //             let new_marker = root.insert(nodes, *object, popped); //index.pop_up(nodes, &object).insert(nodes, *object, popped);

        //             assert_ne!(*marker, new_marker);

        //             // Update marker
        //             *marker = new_marker
        //         }
        //         // else if let Some(child) = node.fits_child(nodes, &object) {
        //         //     eprintln!("Fits in child");
        //         //     index.remove(nodes, object.entity);
        //         //     let new_marker = child.insert(nodes, *object, popped);

        //         //     assert_ne!(*marker, new_marker);

        //         //     // Update marker
        //         //     *marker = new_marker
        //         // }
        //     });

        self.handle_popped(world)?;

        self.register(world);

        self.handle_popped(world)?;

        Ok(())
    }

    pub fn handle_popped(&mut self, world: &mut World) -> Result<(), hecs::ComponentError> {
        let nodes = &mut self.nodes;
        let root = self.root;
        while !self.popped.0.is_empty() {
            let (front, back) = &mut self.popped;

            eprintln!("Handling popped: {:?}", front.len());

            front
                .drain(..)
                .try_for_each(|obj| -> Result<_, hecs::ComponentError> {
                    let mut marker = world.get_mut::<TreeMarker>(obj.entity)?;

                    let new_marker = root.insert(nodes, obj, back);

                    assert_ne!(marker.index, new_marker.index);

                    *marker = new_marker;

                    Ok(())
                })?;

            // Swap buffers and keep going
            mem::swap(&mut self.popped.0, &mut self.popped.1);
        }

        Ok(())
    }

    #[inline]
    pub fn check_collisions<'a, T: Array<Item = &'a Object>>(
        &'a self,
        world: &mut World,
        events: &mut Events,
    ) -> Result<(), hecs::ComponentError> {
        let mut stack = SmallVec::<T>::new();

        self.root
            .check_collisions(world, events, &self.nodes, &mut stack)
    }

    pub fn draw_gizmos(&self, world: &mut World, gizmos: &mut Gizmos) {
        self.root.draw_gizmos(world, &self.nodes, 0, gizmos);
    }
}
