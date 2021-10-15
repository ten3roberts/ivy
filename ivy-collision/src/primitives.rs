use ultraviolet::Vec3;

use crate::CollisionPrimitive;

pub struct Cube {
    pub size: f32,
}

impl Cube {
    pub fn new(size: f32) -> Self {
        Self { size }
    }
}

impl CollisionPrimitive for Cube {
    fn support(&self, dir: Vec3) -> Vec3 {
        let x = if dir.x > 0.0 { self.size } else { -self.size };
        let y = if dir.y > 0.0 { self.size } else { -self.size };
        let z = if dir.z > 0.0 { self.size } else { -self.size };

        Vec3::new(x, y, z)
    }
}

pub struct Sphere {
    pub radius: f32,
}

impl Sphere {
    pub fn new(radius: f32) -> Self {
        Self { radius }
    }
}

impl CollisionPrimitive for Sphere {
    fn support(&self, dir: Vec3) -> Vec3 {
        self.radius * dir
    }
}
