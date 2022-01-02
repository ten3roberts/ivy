use glam::Vec3;

use crate::{Capsule, CollisionPrimitive, Cube, Sphere};

/// Generic collider holding any primitive implementing a support function.
pub struct Collider {
    primitive: Box<dyn CollisionPrimitive + Send + Sync>,
}

impl std::fmt::Debug for Collider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Collider")
            .field("max_radius", &self.primitive.max_radius())
            .finish()
    }
}

impl Collider {
    /// Creates a new collider from arbitrary collision primitive.
    pub fn new<T: 'static + CollisionPrimitive + Send + Sync>(primitive: T) -> Self {
        Self {
            primitive: Box::new(primitive),
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
        Box::new(self.clone())
    }
}

impl Clone for Collider {
    fn clone(&self) -> Self {
        Self {
            primitive: self.primitive.dyn_clone(),
        }
    }
}
