use glam::{Mat4, Vec3};

use crate::{
    util::{minkowski_diff, MAX_ITERATIONS, TOLERANCE},
    CollisionPrimitive, Simplex,
};

/// Performs a gjk intersection test.
/// Returns true if the shapes intersect.
pub fn gjk<A: CollisionPrimitive, B: CollisionPrimitive>(
    a_transform: &Mat4,
    b_transform: &Mat4,
    a_transform_inv: &Mat4,
    b_transform_inv: &Mat4,
    a_coll: &A,
    b_coll: &B,
) -> (bool, Simplex) {
    // Get first support function in direction of separation
    // let dir = (a_pos - b_pos).normalized();
    let dir = Vec3::X;
    let a = minkowski_diff(
        a_transform,
        b_transform,
        a_transform_inv,
        b_transform_inv,
        a_coll,
        b_coll,
        dir,
    );

    let mut simplex = Simplex::Point([a]);

    let mut iterations = 0;
    while let Some(dir) = simplex.next_dir() {
        let dir = dir.normalize();

        // Objects are inside of each other completely
        if dir.length_squared() - 1.0 > TOLERANCE {
            return (false, simplex);
        }

        // Get the next simplex
        let p = minkowski_diff(
            a_transform,
            b_transform,
            a_transform_inv,
            b_transform_inv,
            a_coll,
            b_coll,
            dir,
        );

        // New point was not past the origin
        // No collision
        if iterations > MAX_ITERATIONS || p.support.dot(dir) <= 0.0 {
            return (false, simplex);
        }

        simplex.push(p);
        iterations += 1;
    }

    // Collision found
    (true, simplex)
}
