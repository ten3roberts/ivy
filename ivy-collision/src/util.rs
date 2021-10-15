use std::ops::Deref;

use ultraviolet::{Mat4, Vec3};

use crate::CollisionPrimitive;

pub const TOLERANCE: f32 = 0.01;

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
/// Compute barycentric coordinates of p in relation to the triangle defined by (a, b, c).
pub fn barycentric_vector(p: Vec3, a: Vec3, b: Vec3, c: Vec3) -> (f32, f32, f32) {
    let v0 = b - a;
    let v1 = c - a;
    let v2 = p - a;
    let d00 = v0.dot(v0);
    let d01 = v0.dot(v1);
    let d11 = v1.dot(v1);
    let d20 = v2.dot(v0);
    let d21 = v2.dot(v1);
    let inv_denom = 1.0 / (d00 * d11 - d01 * d01);

    let v = (d11 * d20 - d01 * d21) * inv_denom;
    let w = (d00 * d21 - d01 * d20) * inv_denom;
    let u = 1.0 - v - w;
    (u, v, w)
}
