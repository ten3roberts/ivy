use ivy_core::TransformMatrix;
use ultraviolet::Vec3;

use crate::{Contact, Ray};

pub trait CollisionPrimitive {
    /// Returns the furtherst vertex in `dir`.
    /// Direction is given in collider/model space.
    fn support(&self, dir: Vec3) -> Vec3;
    /// Returns the maximum radius of the primitive. Used for sphere bounding.
    fn max_radius(&self) -> f32;
}

pub trait RayIntersect: CollisionPrimitive + Sized {
    // Returns true if the shape intersects the ray
    fn check_intersect(&self, transform: &TransformMatrix, ray: &Ray) -> bool;
    // Returns the intersection point of a ray onto shape
    fn intersect(&self, transform: &TransformMatrix, ray: &Ray) -> Option<Contact> {
        ray.intersects(self, transform)
    }
}
