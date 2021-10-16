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

    fn max_radius(&self) -> f32 {
        self.size
    }
}

pub struct Sphere {
    pub radius: f32,
}

impl Sphere {
    pub fn new(radius: f32) -> Self {
        Self { radius }
    }

    /// Returns true if two uniform spheres overlap.
    pub fn overlaps(&self, origin: Vec3, other: Self, other_origin: Vec3) -> bool {
        (origin - other_origin).mag_sq()
            < (self.radius * self.radius) + (other.radius * other.radius)
    }
}

impl CollisionPrimitive for Sphere {
    fn support(&self, dir: Vec3) -> Vec3 {
        self.radius * dir
    }

    fn max_radius(&self) -> f32 {
        self.radius
    }
}

pub struct Capsule {
    pub half_height: f32,
    pub radius: f32,
}

impl Capsule {
    pub fn new(half_height: f32, radius: f32) -> Self {
        Self {
            half_height,
            radius,
        }
    }
}

impl CollisionPrimitive for Capsule {
    fn support(&self, dir: Vec3) -> Vec3 {
        let mut result = Vec3::zero();
        result.y = dir.y.signum() * self.half_height;
        result + dir * self.radius
    }

    fn max_radius(&self) -> f32 {
        self.half_height + self.radius
    }
}
