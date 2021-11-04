mod epa;
mod epa2d;
mod epa_ray;
mod polytype;

pub use epa::*;
pub use epa2d::*;
pub use epa_ray::*;
pub(crate) use polytype::*;
use ultraviolet::Vec3;

use crate::{util::plane_ray, Ray};

// Calculates the heuristic distance of a face to a ray
fn ray_distance(p: Vec3, normal: Vec3, ray: &Ray) -> f32 {
    // if normal.dot(ray.dir()) > 0.0 {
    //     f32::MIN
    // } else {
    -plane_ray(p, normal, ray).dot(ray.dir()) * normal.dot(ray.dir()).signum()

    // }
}
