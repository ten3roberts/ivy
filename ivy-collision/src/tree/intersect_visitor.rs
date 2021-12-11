use std::{marker::PhantomData, slice::Iter};

use hecs::Query;
use hecs_schedule::GenericWorld;
use ivy_base::{Position, TransformMatrix};

use crate::{
    intersect, query::TreeQuery, Collider, CollisionObject, CollisionPrimitive, CollisionTree,
    CollisionTreeNode, Contact, Sphere, Visitor,
};

/// Performs intersection testing for a provided temporary collider.
///
/// Use with [crate::CollisionTree::query].
pub struct IntersectVisitor<'a, 'w, W, C, Q = ()> {
    bound: Sphere,
    collider: &'a C,
    origin: Position,
    transform: TransformMatrix,
    world: &'w W,
    with: PhantomData<Q>,
}

impl<'a, 'w, W, C, Q> IntersectVisitor<'a, 'w, W, C, Q>
where
    W: GenericWorld,
    C: CollisionPrimitive,
    Q: Query,
{
    pub fn new(world: &'w W, collider: &'a C, transform: TransformMatrix) -> Self {
        Self {
            bound: Sphere::new(collider.max_radius()),
            collider,
            origin: transform.extract_translation(),
            transform,
            world,
            with: PhantomData,
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
    pub fn intersection<N: CollisionTreeNode>(self, tree: &'a CollisionTree<N>) -> Option<Contact> {
        tree.query(self).flatten().next()
    }
}

impl<'o, 'a, 'w, W: 'a, N, C, Q> Visitor<'o, N> for IntersectVisitor<'a, 'w, W, C, Q>
where
    W: GenericWorld,
    N: CollisionTreeNode,
    C: CollisionPrimitive,
    Q: Query,
{
    type Output = IntersectIterator<'o, 'a, 'w, W, C, Q>;

    fn accept(&self, node: &'o N) -> Option<Self::Output> {
        if node.contains_separate(&self.bound, self.origin) {
            Some(IntersectIterator {
                collider: self.collider,
                transform: self.transform,
                objects: node.objects().iter(),
                bound: self.bound,
                origin: self.origin,
                world: self.world,
                with: PhantomData,
            })
        } else {
            None
        }
    }
}

/// Iterator for object intersection
pub struct IntersectIterator<'o, 'a, 'w, W, C, Q> {
    bound: Sphere,
    collider: &'a C,
    objects: Iter<'o, CollisionObject>,
    origin: Position,
    transform: TransformMatrix,
    world: &'w W,
    with: PhantomData<Q>,
}

impl<'o, 'a, 'w, W, C, Q> Iterator for IntersectIterator<'o, 'a, 'w, W, C, Q>
where
    W: GenericWorld,
    C: CollisionPrimitive,
    Q: Query,
{
    type Item = Contact;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let object = self.objects.next()?;

            if !object
                .bound
                .overlaps(object.origin, &self.bound, self.origin)
            {
                continue;
            }

            let mut query = self
                .world
                .try_query_one::<(&Collider, Q)>(object.entity)
                .expect("Failed to query entity");

            if let Ok((collider, _)) = query.get() {
                if let Some(intersection) = intersect(
                    &object.transform,
                    &self.transform,
                    &*collider,
                    self.collider,
                ) {
                    return Some(intersection);
                }
            };
        }
    }
}
