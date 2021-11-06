use ultraviolet::{Mat4, Vec3};

use crate::{CollisionPrimitive, Ray};

pub const TOLERANCE: f32 = 0.001;
pub const MAX_ITERATIONS: usize = 10;

// Represents a point on the minkowski difference boundary which carries the
// individual support points
#[derive(Default, Debug, Clone, Copy)]
pub struct SupportPoint {
    pub support: Vec3,
    pub a: Vec3,
    pub b: Vec3,
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

    SupportPoint {
        support: a - b,
        a,
        b,
    }
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

/// Gets the normal of a direction vector with a reference point. Normal will
/// face the same direciton as reference
pub fn triple_prod(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    a.cross(b).cross(c).normalized()
}

pub fn project_plane(a: Vec3, normal: Vec3) -> Vec3 {
    a - normal * a.dot(normal)
}

pub fn max_axis(val: Vec3) -> Vec3 {
    if val.x > val.y {
        if val.x > val.z {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 0.0, 1.0)
        }
    } else if val.y > val.z {
        Vec3::new(0.0, 1.0, 0.0)
    } else {
        Vec3::new(0.0, 0.0, 1.0)
    }
}

pub fn max_axis_abs(val: Vec3) -> Vec3 {
    if val.x.abs() > val.y.abs() {
        if val.x > val.z {
            Vec3::new(1.0, 0.0, 0.0)
        } else {
            Vec3::new(0.0, 0.0, 1.0)
        }
    } else if val.y.abs() > val.z.abs() {
        Vec3::new(0.0, 1.0, 0.0)
    } else {
        Vec3::new(0.0, 0.0, 1.0)
    }
}

pub fn plane_ray(p: Vec3, normal: Vec3, ray: &Ray) -> Vec3 {
    plane_intersect(p - *ray.origin(), normal, ray.dir()) + *ray.origin()
}

pub fn plane_intersect(p: Vec3, normal: Vec3, dir: Vec3) -> Vec3 {
    let rel = p;
    let along = -rel.dot(normal);
    let t = -dir.dot(normal);

    along * (dir / t)
}

pub fn edge_intersect(p: Vec3, tangent: Vec3, ray: &Ray) -> Vec3 {
    // Path of the edge point in in tangent plane
    let projected = project_plane(p, tangent);

    ray.dir() * (p.mag() / (projected.dot(ray.dir())))
}

/// Returns an optional intersection between a triangle and a ray
pub fn triangle_ray(points: &[Vec3], ray: &Ray) -> Option<Vec3> {
    let [a, b, c] = [points[0], points[1], points[2]];

    let ab = b - a;
    let ac = c - a;
    let a0 = -a;

    let ab = project_plane(ab, ray.dir());
    let ac = project_plane(ac, ray.dir());
    let a0 = project_plane(a0, ray.dir());

    let perp = triple_prod(ac, ab, ab);

    if perp.dot(a0) > 0.0 {
        return None;
    }
    let perp = triple_prod(ab, ac, ac);

    if perp.dot(a0) > 0.0 {
        return None;
    }

    let normal = (b - a).cross(c - a).normalized();
    Some(plane_ray(a, normal, ray))
}

/// Returns an optional intersection between a triangle and a ray
/// Assumes the points are relative to the ray origin
pub fn triangle_intersect(points: &[Vec3], dir: Vec3) -> Option<Vec3> {
    let [a, b, c] = [points[0], points[1], points[2]];

    let ab = b - a;
    let ac = c - a;
    let a0 = -a;

    let ab = project_plane(ab, dir);
    let ac = project_plane(ac, dir);
    let a0 = project_plane(a0, dir);

    let perp = triple_prod(ac, ab, ab);

    if perp.dot(a0) > 0.0 {
        return None;
    }
    let perp = triple_prod(ab, ac, ac);

    if perp.dot(a0) > 0.0 {
        return None;
    }

    let normal = (b - a).cross(c - a).normalized();
    Some(plane_intersect(a, normal, dir))
}

/// Returns an optional intersection between a triangle and a ray
/// Assumes the points are relative to the ray origin
pub fn check_triangle_intersect(points: &[Vec3], dir: Vec3) -> bool {
    let [a, b, c] = [points[0], points[1], points[2]];

    let ab = b - a;
    let ac = c - a;
    let a0 = -a;

    let ab = project_plane(ab, dir);
    let ac = project_plane(ac, dir);
    let a0 = project_plane(a0, dir);

    let perp = triple_prod(ac, ab, ab);

    if perp.dot(a0) > 0.0 {
        return false;
    }
    let perp = triple_prod(ab, ac, ac);

    if perp.dot(a0) > 0.0 {
        return false;
    }

    true
}

// Calculates the heuristic distance of a face to a ray
pub fn ray_distance(p: SupportPoint, normal: Vec3, ray: &Ray) -> f32 {
    plane_intersect(p.support, normal, ray.dir()).dot(ray.dir()) * -normal.dot(ray.dir()).signum()
}
