use std::marker::PhantomData;

use hecs::{Entity, Query};
use hecs_schedule::GenericWorld;
use ivy_base::{Position, Scale, Visible};
use ordered_float::OrderedFloat;
use ultraviolet::Vec3;

use super::Ray;
use crate::{Collider, CollisionObject, CollisionTreeNode, Contact, Visitor};

/// Represents a collider ray intersection.
/// Data about the ray is not saved.
#[derive(Debug, Clone, PartialEq)]
pub struct RayIntersection {
    pub entity: Entity,
    pub contact: Contact,
}

impl PartialOrd for RayIntersection {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.contact.depth.partial_cmp(&other.contact.depth)
    }
}

impl Eq for RayIntersection {}

impl Ord for RayIntersection {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        OrderedFloat(self.contact.depth).cmp(&OrderedFloat(other.contact.depth))
    }
}

impl RayIntersection {
    pub fn new(entity: Entity, contact: Contact) -> Self {
        Self { entity, contact }
    }

    /// Returns the single ray contact point
    pub fn point(&self) -> Position {
        self.contact.points[0]
    }

    /// Returns the single ray contact point
    pub fn normal(&self) -> Vec3 {
        self.contact.normal
    }
}

/// Visitor for casting a ray into the collision pruning tree
pub struct RayCaster<'r, 'w, W, Q> {
    ray: &'r Ray,
    world: &'w W,
    with: PhantomData<Q>,
}

impl<'r, 'w, Q, W> RayCaster<'r, 'w, W, Q> {
    pub fn new(ray: &'r Ray, world: &'w W) -> Self {
        Self {
            ray,
            world,
            with: PhantomData,
        }
    }
}

impl<'o, 'r, 'w, W: GenericWorld, N: CollisionTreeNode, Q> Visitor<'o, N>
    for RayCaster<'r, 'w, W, Q>
{
    type Output = RayCastIterator<'r, 'w, 'o, W, Q>;

    fn accept(&self, node: &'o N) -> Option<Self::Output> {
        if !node
            .bounds()
            .check_aabb_intersect(node.origin(), Scale(Vec3::one()), self.ray)
        {
            return None;
        }

        let objects = node.objects().iter();
        Some(RayCastIterator {
            ray: self.ray,
            world: self.world,
            objects,
            with: PhantomData,
        })
    }
}
pub struct RayCastIterator<'a, 'w, 'o, W, Q> {
    ray: &'a Ray,
    world: &'w W,
    objects: std::slice::Iter<'o, CollisionObject>,
    with: PhantomData<Q>,
}

/// Query required for ray casting
#[derive(Query)]
pub struct RayCastQuery<'a> {
    collider: &'a Collider,
    visible: &'a Visible,
}

/// Requires collider
impl<'a, 'w, 'o, W: GenericWorld, Q: Query> Iterator for RayCastIterator<'a, 'w, 'o, W, Q> {
    type Item = RayIntersection;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let object = self.objects.next()?;
            if !object
                .bound
                .check_aa_intersect(object.origin.into(), self.ray)
            {
                continue;
            }

            let mut query = self
                .world
                .try_query_one::<(RayCastQuery, Q)>(object.entity)
                .expect("Query failed");

            if let Ok((q, _)) = query.get() {
                if q.visible.is_hidden() {
                    continue;
                }

                if let Some(contact) = self.ray.intersects(&*q.collider, &object.transform) {
                    return Some(RayIntersection::new(object.entity, contact));
                }
            };
        }
    }
}
