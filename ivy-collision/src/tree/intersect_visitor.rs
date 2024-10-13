use std::slice::Iter;

use glam::Mat4;
use slotmap::SlotMap;

use crate::{
    body::{Body, BodyIndex},
    BoundingBox, CollisionTreeNode, Contact, IntersectionGenerator, Shape, TransformedShape,
    TreeVisitor,
};

use super::BvhNode;

/// Performs intersection testing for a provided temporary collider.
///
/// Use with [crate::CollisionTree::query].
pub struct IntersectVisitor<'a, C> {
    bounds: BoundingBox,
    collider: &'a C,
}

impl<'a, C: Shape> IntersectVisitor<'a, C> {
    pub fn new(collider: &'a C) -> Self {
        Self {
            bounds: collider.bounding_box(Mat4::IDENTITY),
            collider,
        }
    }
}

impl<'a, C: Shape> TreeVisitor<'a> for IntersectVisitor<'a, C> {
    type Output = IntersectIterator<'a, C>;

    fn accept(
        &self,
        node: &'a BvhNode,
        data: &'a SlotMap<BodyIndex, Body>,
    ) -> Option<Self::Output> {
        if node.bounds().contains(self.bounds) {
            Some(IntersectIterator {
                collider: self.collider,
                data,
                objects: node.bodies().iter(),
                bounds: self.bounds,
                intersection_generator: Default::default(),
            })
        } else {
            None
        }
    }
}

/// Iterator for object intersection
pub struct IntersectIterator<'a, C> {
    bounds: BoundingBox,
    collider: &'a C,
    objects: Iter<'a, BodyIndex>,
    data: &'a SlotMap<BodyIndex, Body>,
    intersection_generator: IntersectionGenerator,
}

impl<'a, C: Shape> Iterator for IntersectIterator<'a, C> {
    type Item = Contact;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let &object = self.objects.next()?;
            let data = &self.data[object];

            if !data.bounds.overlaps(self.bounds) {
                continue;
            }

            if let Some(contact) = self.intersection_generator.intersect(
                &TransformedShape::new(&data.collider, data.transform),
                &self.collider,
            ) {
                return Some(contact);
            }
        }
    }
}
