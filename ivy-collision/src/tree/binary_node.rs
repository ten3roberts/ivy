use std::{
    iter::{Cloned, Flatten},
    option::Iter,
};

use hecs::Entity;
use ultraviolet::Vec3;

use super::*;
use crate::{util::max_axis, Cube};

#[derive(Debug)]
/// The node in collision tree.
/// Implements basic functionality. Tree behaviours are contained in [`NodeIndex`].
pub struct BinaryNode<T: Array<Item = Object>> {
    // Data for the iteration
    pub(crate) objects: SmallVec<T>,
    pub(crate) entity_count: usize,
    pub(crate) depth: usize,
    pub(crate) iteration: usize,
    pub(crate) origin: Position,
    pub(crate) bounds: Cube,
    pub(crate) children: Option<[NodeIndex; 2]>,
}

impl<T: Array<Item = Object>> std::ops::Deref for BinaryNode<T> {
    type Target = Vec3;

    fn deref(&self) -> &Self::Target {
        &self.origin
    }
}

impl<T: Array<Item = Object>> Node for BinaryNode<T> {
    fn objects(&self) -> &[Object] {
        &self.objects
    }

    fn entity_count(&self) -> usize {
        self.entity_count
    }

    fn contains(&self, object: &Object) -> bool {
        object.origin.x + object.bound.radius < self.origin.x + self.bounds.x
            && object.origin.x - object.bound.radius > self.origin.x - self.bounds.x
            && object.origin.y + object.bound.radius < self.origin.y + self.bounds.y
            && object.origin.y - object.bound.radius > self.origin.y - self.bounds.y
            && object.origin.z + object.bound.radius < self.origin.z + self.bounds.z
            && object.origin.z - object.bound.radius > self.origin.z - self.bounds.z
    }

    fn contains_point(&self, point: Vec3) -> bool {
        point.x < self.origin.x + self.bounds.x
            && point.x > self.origin.x - self.bounds.x
            && point.y < self.origin.y + self.bounds.y
            && point.y > self.origin.y - self.bounds.y
            && point.z < self.origin.z + self.bounds.z
            && point.z > self.origin.z - self.bounds.z
    }

    fn set(&mut self, object: Object, iteration: usize) {
        if iteration != self.iteration {
            self.iteration = iteration;
            // Use set_len since object doesn't implement drop
            unsafe { self.objects.set_len(0) }
        }
        self.objects.push(object);
        assert!(self.objects.len() <= self.entity_count);
    }

    fn try_add(&mut self, object: Object) -> Result<(), Object> {
        // Node is not already split and full
        if self.is_leaf() && self.entity_count >= self.objects.inline_size() {
            Err(object)
        } else {
            self.objects.push(object);
            self.entity_count += 1;
            Ok(())
        }
    }

    fn remove(&mut self, entity: Entity) -> Option<Object> {
        if let Some((index, _)) = self
            .objects
            .iter()
            .enumerate()
            .find(|(_, val)| val.entity == entity)
        {
            let obj = Some(self.objects.swap_remove(index));
            // self.objects.shrink_to_fit();
            self.entity_count += 1;
            obj
        } else {
            None
        }
    }

    fn origin(&self) -> Position {
        self.origin
    }

    fn bounds(&self) -> Cube {
        self.bounds
    }

    fn children(&self) -> &[NodeIndex] {
        match &self.children {
            Some(val) => val,
            None => &[],
        }
    }

    fn is_leaf(&self) -> bool {
        self.children.is_none()
    }

    type SplitOutput = [Self; 2];

    fn split(&mut self, popped: &mut Vec<Object>) -> Self::SplitOutput {
        // eprintln!("Splitting");
        let mut center = Vec3::zero();
        let mut max = Vec3::zero();
        let mut min = Vec3::zero();

        eprintln!("Children: {:?}", self.children);
        assert!(self.children.is_none());

        self.objects.iter().for_each(|val| {
            center += val.origin;
            max = max.max_by_component(val.origin);
            min = min.min_by_component(val.origin);
        });

        let width = (max - min).abs();

        let max = max_axis(width);

        let off = *self.bounds * max * 0.5;
        let origin = self.origin;

        let extents = *self.bounds - off;
        let a_origin = *origin - off;
        let b_origin = *origin + off;

        let a = BinaryNode::new(self.depth + 1, a_origin.into(), Cube::new(extents));
        let b = BinaryNode::new(self.depth + 1, b_origin.into(), Cube::new(extents));

        // Repartition selfs. Retain those that do not fit in any new leaf, and
        // push those that do to the popped list.

        self.clear().for_each(|val| popped.push(val));

        [a, b]
    }

    fn set_children(&mut self, children: &[NodeIndex]) {
        self.children = Some([children[0], children[1]])
    }
}

impl<T: Array<Item = Object>> BinaryNode<T> {
    pub fn new(depth: usize, origin: Position, bounds: Cube) -> Self {
        Self {
            objects: Default::default(),
            entity_count: 0,
            depth,
            iteration: 0,
            origin,
            bounds,
            children: None,
        }
    }

    /// Returns the child that fully contains object, if any.
    pub fn fits_child(&self, nodes: &Nodes<Self>, object: &Object) -> Option<NodeIndex> {
        self.children_iter()
            .find(|val| nodes[*val].contains(object))
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
        self.entity_count = 0;
        self.objects.drain(..)
    }

    /// Returns the node's children. If the node is a leaf node, and empty slice
    /// is returned
    pub fn children_iter(&self) -> Cloned<Flatten<Iter<[NodeIndex; 2]>>> {
        self.children.iter().flatten().cloned()
    }
}
