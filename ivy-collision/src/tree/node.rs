use arrayvec::ArrayVec;
use hecs::Entity;
use ivy_core::TransformMatrix;
use ultraviolet::Vec3;

use super::*;
use crate::Sphere;

#[derive(Debug)]
/// The node in collision tree.
/// Implements basic functionality. Tree behaviours are contained in [`NodeIndex`].
pub struct Node<const CAP: usize> {
    // Data for the iteration
    pub(crate) objects: ArrayVec<Object, CAP>,
    pub(crate) depth: usize,
    pub(crate) iteration: usize,
    pub(crate) origin: Vec3,
    pub(crate) half_extents: Vec3,
    pub(crate) children: Option<[NodeIndex; 2]>,
    pub(crate) parent: NodeIndex,
}

impl<const CAP: usize> std::ops::Deref for Node<CAP> {
    type Target = Vec3;

    fn deref(&self) -> &Self::Target {
        &self.origin
    }
}

impl<const CAP: usize> Node<CAP> {
    pub fn new(parent: NodeIndex, depth: usize, origin: Vec3, half_extents: Vec3) -> Self {
        Self {
            objects: ArrayVec::default(),
            depth,
            iteration: 0,
            origin,
            half_extents,
            children: None,
            parent,
        }
    }

    /// Returns the child that fully contains object, if any.
    pub fn fits_child(&self, nodes: &Nodes<CAP>, object: &Object) -> Option<NodeIndex> {
        self.children
            .iter()
            .flatten()
            .find(|val| nodes[**val].contains(object))
            .map(|val| *val)
    }

    /// Returns true if the object bounded by a sphere fits in the node.
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
            self.objects.clear();
        }
        self.objects.push(object);
    }

    /// Adds an entity
    /// Panics if the node can not fit any more entities
    #[inline]
    pub fn push(&mut self, object: Object) {
        self.objects.push(object)
    }

    /// Removes an entity
    #[inline]
    pub fn remove(&mut self, entity: Entity) {
        if let Some((index, _)) = self
            .objects
            .iter()
            .enumerate()
            .find(|(_, val)| val.entity == entity)
        {
            self.objects.swap_remove(index);
        } else {
            panic!("Can not remove entity not in node");
        }
    }

    #[inline]
    pub fn set_children(&mut self, children: [NodeIndex; 2]) {
        assert_eq!(self.children, None);
        self.children = Some(children);
    }

    pub fn remaining_capacity(&self) -> usize {
        self.objects.remaining_capacity()
    }

    /// Get a mutable reference to the node's entities.
    pub fn objects_mut(&mut self) -> &mut ArrayVec<Object, CAP> {
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
