use std::slice::Iter;

use flax::{Fetch, World};
use glam::Mat4;
use slotmap::SlotMap;

use crate::{
    components, contact::ContactSurface, query::TreeQuery, Body, BodyIndex, BoundingBox,
    CollisionTree, CollisionTreeNode, IntersectionGenerator, Shape, TransformedShape, Visitor,
};

use super::BvhNode;

/// Performs intersection testing for a provided temporary collider.
///
/// Use with [crate::CollisionTree::query].
pub struct IntersectVisitor<'a, C, Q> {
    bounds: BoundingBox,
    collider: &'a C,
    transform: Mat4,
    world: &'a World,
    filter: &'a Q,
}

impl<'a, C, Q> IntersectVisitor<'a, C, Q>
where
    C: Shape,
{
    pub fn new(world: &'a World, collider: &'a C, transform: Mat4, filter: &'a Q) -> Self
    where
        Q: 'a + for<'x> Fetch<'x>,
    {
        Self {
            bounds: collider.bounding_box(transform),
            collider,
            transform,
            world,
            filter,
        }
    }

    /// Returns all intersections
    pub fn intersections(self, tree: &'a CollisionTree) -> TreeQuery<Self> {
        tree.query(self)
    }
    /// Returns the first intersection, by no order.
    pub fn intersection(self, tree: &'a CollisionTree) -> Option<ContactSurface>
    where
        Q: for<'x> Fetch<'x>,
    {
        tree.query(self).flatten().next()
    }
}

impl<'a, C, Q> Visitor<'a> for IntersectVisitor<'a, C, Q>
where
    C: Shape,
    Q: 'a,
{
    type Output = IntersectIterator<'a, C, Q>;

    fn accept(
        &self,
        node: &'a BvhNode,
        data: &'a SlotMap<BodyIndex, Body>,
    ) -> Option<Self::Output> {
        if node.bounds().contains(self.bounds) {
            Some(IntersectIterator {
                collider: self.collider,
                transform: self.transform,
                data,
                objects: node.objects().iter(),
                bounds: self.bounds,
                world: self.world,
                filter: self.filter,
                intersection_generator: Default::default(),
            })
        } else {
            None
        }
    }
}

/// Iterator for object intersection
pub struct IntersectIterator<'a, C, Q> {
    bounds: BoundingBox,
    collider: &'a C,
    objects: Iter<'a, BodyIndex>,
    data: &'a SlotMap<BodyIndex, Body>,
    transform: Mat4,
    world: &'a World,
    filter: &'a Q,
    intersection_generator: IntersectionGenerator,
}

impl<'a, C, Q> Iterator for IntersectIterator<'a, C, Q>
where
    C: Shape,
    Q: for<'x> Fetch<'x>,
{
    type Item = ContactSurface;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let &object = self.objects.next()?;
            let data = &self.data[object];

            if !data.bounds.overlaps(self.bounds) {
                continue;
            }

            let entity = self.world.entity(data.id).expect("Invalid entity in tree");

            let query = (components::collider(), self.filter);

            if let Some((collider, _)) = entity.query(&query).get() {
                if let Some(intersection) = self.intersection_generator.intersect(
                    &TransformedShape::new(&collider, data.transform),
                    &TransformedShape::new(&self.collider, self.transform),
                ) {
                    return Some(intersection);
                }
            };
        }
    }
}
