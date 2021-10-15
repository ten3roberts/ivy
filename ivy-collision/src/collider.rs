use ultraviolet::Vec3;

use crate::CollisionPrimitive;

/// Generic collider holding any primitive implementing a support function.
pub struct Collider {
    primitive: Box<dyn CollisionPrimitive + Send + Sync>,
}

impl Collider {
    /// Creates a new collider from collision primitive.
    pub fn new<T: 'static + CollisionPrimitive + Send + Sync>(primitive: T) -> Self {
        Self {
            primitive: Box::new(primitive),
        }
    }
}

impl CollisionPrimitive for Collider {
    fn support(&self, dir: Vec3) -> Vec3 {
        self.primitive.support(dir)
    }
}
