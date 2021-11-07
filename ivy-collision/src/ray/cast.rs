use hecs::{Entity, World};
use ivy_base::Scale;
use ultraviolet::Vec3;

use super::Ray;
use crate::{Collider, Contact, Node, Object, Visitor};

/// Represents a collider ray intersection.
/// Data about the ray is not saved.
pub struct RayIntersection {
    pub entity: Entity,
    pub contact: Contact,
}

impl RayIntersection {
    pub fn new(entity: Entity, contact: Contact) -> Self {
        Self { entity, contact }
    }

    /// Returns the single ray contact point
    pub fn point(&self) -> Vec3 {
        self.contact.points[0]
    }
}

/// Visitor for casting a ray into the collision pruning tree
pub struct RayCaster<'r, 'w> {
    ray: &'r Ray,
    world: &'w World,
}

impl<'r, 'w> RayCaster<'r, 'w> {
    pub fn new(ray: &'r Ray, world: &'w World) -> Self {
        Self { ray, world }
    }
}

impl<'o, 'r, 'w, N: Node> Visitor<'o, N> for RayCaster<'r, 'w> {
    type Output = RayCastIterator<'r, 'w, 'o>;

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
        })
    }
}
pub struct RayCastIterator<'a, 'w, 'o> {
    ray: &'a Ray,
    world: &'w World,
    objects: std::slice::Iter<'o, Object>,
}

impl<'a, 'w, 'o> Iterator for RayCastIterator<'a, 'w, 'o> {
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

            let collider = self.world.get::<Collider>(object.entity).ok()?;
            if let Some(contact) = self.ray.intersects(&*collider, &object.transform) {
                return Some(RayIntersection::new(object.entity, contact));
            }
        }
    }
}
