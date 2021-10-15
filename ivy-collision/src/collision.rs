use ultraviolet::Mat4;

use crate::{epa, gjk, CollisionPrimitive, Intersection};

pub fn intersect<T: CollisionPrimitive>(
    a_transform: &Mat4,
    b_transform: &Mat4,
    a: &T,
    b: &T,
) -> Option<Intersection> {
    let a_transform_inv = a_transform.inversed();
    let b_transform_inv = b_transform.inversed();

    let (intersect, simplex) = gjk(
        a_transform,
        b_transform,
        &a_transform_inv,
        &b_transform_inv,
        a,
        b,
    );

    if intersect {
        Some(epa(
            a_transform,
            b_transform,
            &a_transform_inv,
            &b_transform_inv,
            a,
            b,
            simplex,
        ))
    } else {
        None
    }
}
