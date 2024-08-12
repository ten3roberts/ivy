use std::slice::Iter;

use flax::{Fetch, World};
use glam::Mat4;
use slotmap::SlotMap;

use crate::{
    components, intersect, query::TreeQuery, BoundingBox, CollisionPrimitive, CollisionTree,
    CollisionTreeNode, Contact, ObjectData, ObjectIndex, Visitor,
};

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
    C: CollisionPrimitive,
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
    pub fn intersections<N: CollisionTreeNode>(
        self,
        tree: &'a CollisionTree<N>,
    ) -> TreeQuery<N, Self> {
        tree.query(self)
    }
    /// Returns the first intersection, by no order.
    pub fn intersection<N: CollisionTreeNode>(self, tree: &'a CollisionTree<N>) -> Option<Contact>
    where
        Q: for<'x> Fetch<'x>,
    {
        tree.query(self).flatten().next()
    }
}

impl<'a, N, C, Q> Visitor<'a, N> for IntersectVisitor<'a, C, Q>
where
    N: CollisionTreeNode,
    C: CollisionPrimitive,
    Q: 'a,
{
    type Output = IntersectIterator<'a, C, Q>;

    fn accept(
        &self,
        node: &'a N,
        data: &'a SlotMap<ObjectIndex, ObjectData>,
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
    objects: Iter<'a, ObjectIndex>,
    data: &'a SlotMap<ObjectIndex, ObjectData>,
    transform: Mat4,
    world: &'a World,
    filter: &'a Q,
}

impl<'a, C, Q> Iterator for IntersectIterator<'a, C, Q>
where
    C: CollisionPrimitive,
    Q: for<'x> Fetch<'x>,
{
    type Item = Contact;

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
                if let Some(intersection) =
                    intersect(&data.transform, &self.transform, collider, self.collider)
                {
                    return Some(intersection);
                }
            };
        }
    }
}
