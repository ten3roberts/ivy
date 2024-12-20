use std::option::Option;

use glam::Vec3;
use rapier3d::math::DEFAULT_EPSILON;

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
