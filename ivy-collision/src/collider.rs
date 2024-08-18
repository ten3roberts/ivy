use derive_more::{Deref, DerefMut, Display, From, Into};
use ezy::Lerp;
use glam::{Mat4, Vec3};

use crate::{BoundingBox, Capsule, Shape, Sphere};

#[derive(Debug, Display, Clone, Copy, Deref, DerefMut, Default, From, Into)]
pub struct ColliderOffset(pub Mat4);

impl<'a> Lerp<'a> for ColliderOffset {
    type Write = &'a mut Self;

    fn lerp(_write: Self::Write, _start: &Self, _end: &Self, _t: f32) {
        unimplemented!()
    }
}

/// Generic collider holding any primitive implementing a support function.
#[derive(Debug, Clone)]
pub enum Collider {
    Cube(BoundingBox),
    Sphere(Sphere),
    Capsule(Capsule),
}

impl Collider {
    /// Creates a cuboidal collider
    pub fn cube(min: Vec3, max: Vec3) -> Self {
        Self::Cube(BoundingBox::from_corners(min, max))
    }

    pub fn cube_from_center(center: Vec3, half_extents: Vec3) -> Self {
        Self::Cube(BoundingBox::from_corners(
            center - half_extents,
            center + half_extents,
        ))
    }

    /// Creates a spherical collider
    pub fn sphere(radius: f32) -> Self {
        Self::Sphere(Sphere::new(radius))
    }

    /// Creates a capsule collider
    pub fn capsule(half_height: f32, radius: f32) -> Self {
        Self::Capsule(Capsule::new(half_height, radius))
    }
}

impl Shape for Collider {
    fn support(&self, dir: Vec3) -> Vec3 {
        match self {
            Collider::Cube(v) => v.support(dir),
            Collider::Sphere(v) => v.support(dir),
            Collider::Capsule(v) => v.support(dir),
        }
    }

    fn max_radius(&self) -> f32 {
        match self {
            Collider::Cube(v) => v.max_radius(),
            Collider::Sphere(v) => v.max_radius(),
            Collider::Capsule(v) => v.max_radius(),
        }
    }
}
