use ultraviolet::Vec3;

use crate::{CollisionPrimitive, Cube};

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
    /// Creates a new collider from collision primitive.
    pub fn new<T: 'static + CollisionPrimitive + Send + Sync + Clone>(primitive: T) -> Self {
        Self {
            primitive: Box::new(primitive),
        }
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
}
