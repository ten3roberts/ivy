use ivy_core::Scale;
use ultraviolet::Vec3;

use crate::CollisionPrimitive;

#[derive(Debug, Clone, Copy, PartialEq)]
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
        let sq = self.size * self.size;
        (3.0 * sq).sqrt()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    pub radius: f32,
}

impl Sphere {
    pub fn new(radius: f32) -> Self {
        Self { radius }
    }

    /// Returns true if two uniform spheres overlap.
    #[inline]
    pub fn overlaps(&self, origin: Vec3, other: Self, other_origin: Vec3) -> bool {
        let total_radii = self.radius + other.radius;

        (origin - other_origin).mag_sq() < total_radii * total_radii
    }

    /// Creates a bounding sphere fully enclosign a primitive
    #[inline]
    pub fn enclose<T: CollisionPrimitive>(collider: &T, scale: Scale) -> Self {
        Self {
            radius: collider.max_radius() * scale.component_max(),
        }
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

#[derive(Debug, Clone, Copy, PartialEq)]
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
