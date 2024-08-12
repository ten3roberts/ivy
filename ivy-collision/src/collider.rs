use std::sync::Arc;

use derive_more::{Deref, DerefMut, Display, From, Into};
use ezy::Lerp;
use glam::{Mat4, Vec3};

use crate::{Capsule, CollisionPrimitive, Cube, Sphere};

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
pub struct Collider {
    // TODO: enum
    primitive: Arc<dyn CollisionPrimitive + Send + Sync>,
}

impl Collider {
    /// Creates a new collider from arbitrary collision primitive.
    pub fn new<T: 'static + CollisionPrimitive + Send + Sync>(primitive: T) -> Self {
        Self {
            primitive: Arc::new(primitive),
        }
    }

    /// Creates a cuboidal collider
    pub fn cube(x: f32, y: f32, z: f32) -> Self {
        Self::new(Cube::new(x, y, z))
    }

    /// Creates a spherical collider
    pub fn sphere(radius: f32) -> Self {
        Self::new(Sphere::new(radius))
    }

    /// Creates a capsule collider
    pub fn capsule(half_height: f32, radius: f32) -> Self {
        Self::new(Capsule::new(half_height, radius))
    }
}

impl Default for Collider {
    fn default() -> Self {
        Self::new(Cube::uniform(1.0))
    }
}

impl CollisionPrimitive for Collider {
    fn support(&self, dir: Vec3) -> Vec3 {
        self.primitive.support(dir)
    }

    fn max_radius(&self) -> f32 {
        self.primitive.max_radius()
    }

    fn dyn_clone(&self) -> Box<dyn CollisionPrimitive + Send + Sync> {
        unimplemented!()
    }
}
