use std::slice::Iter;

use flax::Entity;
use glam::{f32, Vec3};
use slotmap::SlotMap;

use super::Ray;
use crate::{
    body::{Body, BodyIndex},
    BvhNode, CollisionTreeNode, Contact, TreeVisitor,
};

#[derive(Debug, Clone)]
pub struct RayIntersection {
    pub body: BodyIndex,
    pub id: Entity,
    pub contact: Contact,
}

impl RayIntersection {
    /// Returns the single ray contact point
    pub fn point(&self) -> Vec3 {
        self.contact.point_a
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
pub struct RayCaster {
    ray: Ray,
}

impl RayCaster {
    pub fn new(ray: Ray) -> Self {
        Self { ray }
    }
}

impl<'a> TreeVisitor<'a> for RayCaster {
    type Output = RayCastIterator<'a>;

    fn accept(
        &self,
        node: &'a BvhNode,
        data: &'a SlotMap<BodyIndex, Body>,
    ) -> Option<Self::Output> {
        if !node.bounds().check_ray(&self.ray) {
            return None;
        }

        let bodies = node.bodies().iter();
        Some(RayCastIterator {
            ray: self.ray,
            data,
            bodies,
        })
    }
}

pub struct RayCastIterator<'a> {
    ray: Ray,
    bodies: Iter<'a, BodyIndex>,
    data: &'a SlotMap<BodyIndex, Body>,
}

impl<'a> Iterator for RayCastIterator<'a> {
    type Item = RayIntersection;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let &body_index = self.bodies.next()?;
            let data = &self.data[body_index];

            if !data.bounds.check_ray(&self.ray) {
                continue;
            }

            if let Some(contact) = self.ray.intersect(&data.collider, &data.transform) {
                return Some(RayIntersection {
                    body: body_index,
                    id: data.id,
                    contact,
                });
            }
        }
    }
}
