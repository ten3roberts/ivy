use hecs::Entity;
use ivy_core::TransformMatrix;
use ultraviolet::Vec3;

use super::*;
use crate::Sphere;

#[derive(Debug)]
/// The node in collision tree.
/// Implements basic functionality. Tree behaviours are contained in [`NodeIndex`].
pub struct Node<T: Array<Item = Object>> {
    // Data for the iteration
    pub(crate) objects: SmallVec<T>,
    pub(crate) object_count: usize,
    pub(crate) depth: usize,
    pub(crate) iteration: usize,
    pub(crate) origin: Vec3,
    pub(crate) half_extents: Vec3,
    pub(crate) children: Option<[NodeIndex; 2]>,
    pub(crate) parent: NodeIndex,
}

impl<T: Array<Item = Object>> std::ops::Deref for Node<T> {
    type Target = Vec3;

    fn deref(&self) -> &Self::Target {
        &self.origin
    }
}

impl<T: Array<Item = Object>> Node<T> {
    pub fn new(parent: NodeIndex, depth: usize, origin: Vec3, half_extents: Vec3) -> Self {
        Self {
            objects: Default::default(),
            object_count: 0,
            depth,
            iteration: 0,
            origin,
            half_extents,
            children: None,
            parent,
        }
    }

    /// Returns the child that fully contains object, if any.
    pub fn fits_child(&self, nodes: &Nodes<T>, object: &Object) -> Option<NodeIndex> {
        self.children
            .iter()
            .flatten()
            .find(|val| nodes[**val].contains(object))
            .map(|val| *val)
    }

    /// Returns true if the point is inside the node
    pub fn contains_point(&self, point: Vec3) -> bool {
        point.x < self.origin.x + self.half_extents.x
            && point.x > self.origin.x - self.half_extents.x
            && point.y < self.origin.y + self.half_extents.y
            && point.y > self.origin.y - self.half_extents.y
            && point.z < self.origin.z + self.half_extents.z
            && point.z > self.origin.z - self.half_extents.z
    }

    /// Returns true if the object bounded by a sphere completely fits in the node.
    pub fn contains(&self, object: &Object) -> bool {
        object.origin.x + object.bound.radius < self.origin.x + self.half_extents.x
            && object.origin.x - object.bound.radius > self.origin.x - self.half_extents.x
            && object.origin.y + object.bound.radius < self.origin.y + self.half_extents.y
            && object.origin.y - object.bound.radius > self.origin.y - self.half_extents.y
            && object.origin.z + object.bound.radius < self.origin.z + self.half_extents.z
            && object.origin.z - object.bound.radius > self.origin.z - self.half_extents.z
    }

    /// Sets the value of an object for this iteration
    #[inline]
    pub fn set(&mut self, object: Object, iteration: usize) {
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
    pub fn push(&mut self, object: Object) {
        self.objects.push(object);
        self.object_count += 1;
    }

    /// Removes an entity
    #[inline]
    pub fn remove(&mut self, entity: Entity) -> Option<Object> {
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

    #[inline]
    pub fn set_children(&mut self, children: [NodeIndex; 2]) {
        assert_eq!(self.children, None);
        self.children = Some(children);
    }

    /// Returns the remaining inline capacity. Returns None if data has been
    /// spilled.
    pub fn remaining_capacity(&self) -> Option<usize> {
        let inline_size = self.objects.inline_size();
        let len = self.objects.len();

        if inline_size >= len {
            Some(inline_size - len)
        } else {
            None
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
