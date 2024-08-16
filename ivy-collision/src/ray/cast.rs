use std::slice::Iter;

use flax::{Entity, Fetch, World};
use glam::{f32, Vec3};
use ordered_float::OrderedFloat;
use slotmap::SlotMap;

use super::Ray;
use crate::{
    components::collider, BvhNode, CollisionTreeNode, Contact, ObjectData, ObjectIndex, Visitor,
};

/// Represents a collider ray intersection.
/// Data about the ray is not saved.
#[derive(Debug, Clone)]
pub struct RayIntersection {
    pub id: Entity,
    pub contact: Contact,
}

impl RayIntersection {
    pub fn new(entity: Entity, contact: Contact) -> Self {
        Self {
            id: entity,
            contact,
        }
    }

    /// Returns the single ray contact point
    pub fn point(&self) -> Vec3 {
        self.contact.points[0]
    }

    /// Returns the single ray contact point
    pub fn normal(&self) -> Vec3 {
        self.contact.normal
    }

    /// Returns the intersection distance
    pub fn distance(&self) -> f32 {
        self.contact.depth
    }
}

/// Visitor for casting a ray into the collision pruning tree
pub struct RayCaster<'a, Q> {
    ray: &'a Ray,
    world: &'a World,
    filter: &'a Q,
}

impl<'a, Q> RayCaster<'a, Q> {
    pub fn new(ray: &'a Ray, world: &'a World, filter: &'a Q) -> Self {
        Self { ray, world, filter }
    }
}

impl<'a, Q: 'a> Visitor<'a> for RayCaster<'a, Q> {
    type Output = RayCastIterator<'a, Q>;

    fn accept(
        &self,
        node: &'a BvhNode,
        data: &'a SlotMap<ObjectIndex, ObjectData>,
    ) -> Option<Self::Output> {
        if !node.bounds().check_ray(self.ray) {
            return None;
        }

        let objects = node.objects().iter();
        Some(RayCastIterator {
            ray: self.ray,
            world: self.world,
            data,
            objects,
            filter: self.filter,
        })
    }
}
pub struct RayCastIterator<'a, Q> {
    ray: &'a Ray,
    world: &'a World,
    objects: Iter<'a, ObjectIndex>,
    data: &'a SlotMap<ObjectIndex, ObjectData>,
    filter: &'a Q,
}

/// Requires collider
impl<'a, Q: for<'x> Fetch<'x>> Iterator for RayCastIterator<'a, Q> {
    type Item = RayIntersection;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let &object = self.objects.next()?;
            let data = &self.data[object];

            if !data.bounds.check_ray(self.ray) {
                continue;
            }

            let query = &(collider(), self.filter);

            let entity = self.world.entity(data.id).unwrap();

            if let Some((collider, _)) = entity.query(query).get() {
                // TODO
                // if q.visible.is_hidden() {
                //     continue;
                // }

                if let Some(contact) = self.ray.intersects(collider, &data.transform) {
                    return Some(RayIntersection::new(data.id, contact));
                }
            };
        }
    }
}
