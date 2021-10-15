use std::ops::Deref;

use ultraviolet::{Mat4, Vec3};

pub const TOLERANCE: f32 = 0.01;

pub trait CollisionPrimitive {
    /// Returns the furtherst vertex in `dir`.
    /// Direction is given in collider/model space.
    fn support(&self, dir: Vec3) -> Vec3;
}

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

// Represents a point on the minkowski difference boundary which carries the
// individual support points
#[derive(Debug, Clone, Copy)]
pub struct SupportPoint {
    pub pos: Vec3,
    pub a: Vec3,
    pub b: Vec3,
}

impl Deref for SupportPoint {
    type Target = Vec3;

    fn deref(&self) -> &Self::Target {
        &self.pos
    }
}

/// Returns a point on the minkowski difference given from two colliders, their
/// transform, and a direction.
#[inline]
pub fn minkowski_diff<A: CollisionPrimitive, B: CollisionPrimitive>(
    a_transform: &Mat4,
    b_transform: &Mat4,
    a_transform_inv: &Mat4,
    b_transform_inv: &Mat4,
    a_coll: &A,
    b_coll: &B,
    dir: Vec3,
) -> SupportPoint {
    let a = support(a_transform, a_transform_inv, a_coll, dir);
    let b = support(b_transform, b_transform_inv, b_coll, -dir);

    SupportPoint { pos: a - b, a, b }
}

#[inline]
pub fn support<T: CollisionPrimitive>(
    transform: &Mat4,
    transform_inv: &Mat4,
    coll: &T,
    dir: Vec3,
) -> Vec3 {
    transform.transform_point3(coll.support(transform_inv.transform_vec3(dir).normalized()))
}
