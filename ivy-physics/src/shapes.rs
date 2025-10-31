use std::option::Option;

use glam::Vec3;
use rapier3d::{math::DEFAULT_EPSILON, parry};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    normal: Vec3,
    distance: f32,
}

impl Plane {
    pub fn new(normal: Vec3, distance: f32) -> Self {
        Self { normal, distance }
    }

    pub fn from_normal_and_point(normal: Vec3, point: Vec3) -> Self {
        Self {
            normal,
            distance: normal.dot(point),
        }
    }

    pub fn intersect_ray(&self, ray_origin: Vec3, ray_direction: Vec3) -> Option<f32> {
        let denom = self.normal.dot(ray_direction);
        if denom.abs() > DEFAULT_EPSILON {
            let t = (self.normal * self.distance - ray_origin).dot(self.normal) / denom;
            if t >= 0.0 {
                return Some(t);
            }
        }

        None
    }
}

pub use parry::query::Ray as ParryRay;

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

impl Ray {
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self { origin, direction }
    }

    pub fn origin(&self) -> Vec3 {
        self.origin
    }

    pub fn direction(&self) -> Vec3 {
        self.direction
    }

    pub fn at(&self, t: f32) -> Vec3 {
        self.origin + self.direction * t
    }
}

impl From<(Vec3, Vec3)> for Ray {
    fn from(tuple: (Vec3, Vec3)) -> Self {
        Self::new(tuple.0, tuple.1)
    }
}

impl From<Ray> for ParryRay {
    fn from(ray: Ray) -> Self {
        parry::query::Ray::new(ray.origin.into(), ray.direction.into())
    }
}

impl From<ParryRay> for Ray {
    fn from(ray: parry::query::Ray) -> Self {
        Self::new(ray.origin.into(), ray.dir.into())
    }
}
