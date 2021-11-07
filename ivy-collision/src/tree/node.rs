use std::{
    iter::{Cloned, Flatten},
    option::Iter,
};

use hecs::Entity;
use ivy_base::TransformMatrix;
use ultraviolet::Vec3;

use super::*;
use crate::{Cube, Sphere};

#[derive(Debug)]
/// The node in collision tree.
/// Implements basic functionality. Tree behaviours are contained in [`NodeIndex`].
pub struct BinaryNode<T: Array<Item = Object>> {
    // Data for the iteration
    pub(crate) objects: SmallVec<T>,
    pub(crate) object_count: usize,
    pub(crate) depth: usize,
    pub(crate) iteration: usize,
    pub(crate) origin: Position,
    pub(crate) bounds: Cube,
    pub(crate) children: Option<[NodeIndex; 2]>,
    pub(crate) parent: NodeIndex,
}

impl<T: Array<Item = Object>> std::ops::Deref for BinaryNode<T> {
    type Target = Vec3;

    fn deref(&self) -> &Self::Target {
        &self.origin
    }
}

impl<T: Array<Item = Object>> BinaryNode<T> {
    pub fn new(parent: NodeIndex, depth: usize, origin: Position, bounds: Cube) -> Self {
        Self {
            objects: Default::default(),
            object_count: 0,
            depth,
            iteration: 0,
            origin,
            bounds,
            children: None,
            parent,
        }
    }

    /// Returns true if the node is filled to capacity
    pub fn full(&self) -> bool {
        let inline_size = self.objects.inline_size();

        let len = self.objects.len();

        len >= inline_size
    }

    /// Clears the node objects and returns an iterator over the cleared items.
    pub fn clear(&mut self) -> smallvec::Drain<T> {
        self.object_count = 0;
        self.objects.drain(..)
    }

    /// Get a mutable reference to the node's entities.
    pub fn objects_mut(&mut self) -> &mut SmallVec<T> {
        &mut self.objects
    }
}

impl<T: Array<Item = Object>> Node for BinaryNode<T> {
    /// Returns true if the point is inside the node
    fn contains_point(&self, point: Vec3) -> bool {
        point.x < self.origin.x + self.bounds.x
            && point.x > self.origin.x - self.bounds.x
            && point.y < self.origin.y + self.bounds.y
            && point.y > self.origin.y - self.bounds.y
            && point.z < self.origin.z + self.bounds.z
            && point.z > self.origin.z - self.bounds.z
    }

    /// Returns true if the object bounded by a sphere completely fits in the node.
    fn contains(&self, object: &Object) -> bool {
        object.origin.x + object.bound.radius < self.origin.x + self.bounds.x
            && object.origin.x - object.bound.radius > self.origin.x - self.bounds.x
            && object.origin.y + object.bound.radius < self.origin.y + self.bounds.y
            && object.origin.y - object.bound.radius > self.origin.y - self.bounds.y
            && object.origin.z + object.bound.radius < self.origin.z + self.bounds.z
            && object.origin.z - object.bound.radius > self.origin.z - self.bounds.z
    }

    /// Sets the value of an object for this iteration
    #[inline]
    fn set(&mut self, object: Object, iteration: usize) {
        if iteration != self.iteration {
            self.iteration = iteration;
            // Use set_len since object doesn't implement drop
            unsafe { self.objects.set_len(0) }
        }
        self.objects.push(object);
        assert!(self.objects.len() <= self.object_count);
    }

    /// Adds an entity
    #[inline]
    fn add(&mut self, object: Object) {
        self.objects.push(object);
        self.object_count += 1;
    }

    /// Removes an entity
    #[inline]
    fn remove(&mut self, entity: Entity) -> Option<Object> {
        if let Some((index, _)) = self
            .objects
            .iter()
            .enumerate()
            .find(|(_, val)| val.entity == entity)
        {
            let obj = Some(self.objects.swap_remove(index));
            // self.objects.shrink_to_fit();
            self.object_count += 1;
            obj
        } else {
            None
        }
    }

    /// Get a reference to the node's origin.
    fn origin(&self) -> Position {
        self.origin
    }

    /// Get a reference to the node's bounds.
    fn bounds(&self) -> Cube {
        self.bounds
    }

    /// Returns the node's children. If the node is a leaf node, and empty slice
    /// is returned
    fn children(&self) -> &[NodeIndex] {
        match &self.children {
            Some(val) => val,
            None => &[],
        }
    }

    /// Returns the node's children. If the node is a leaf node, and empty slice
    /// is returned
    fn children_iter(&self) -> Cloned<Flatten<Iter<[NodeIndex; 2]>>> {
        self.children.iter().flatten().cloned()
    }

    fn is_leaf(&self) -> bool {
        self.children.is_none()
    }

    /// Get a reference to the node's parent.
    fn parent(&self) -> NodeIndex {
        self.parent
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
