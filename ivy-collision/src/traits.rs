use ivy_base::TransformMatrix;
use ultraviolet::Vec3;

use crate::{Contact, Ray};

pub trait CollisionPrimitive {
    /// Returns the furtherst vertex in `dir`.
    /// Direction is given in collider/model space.
    fn support(&self, dir: Vec3) -> Vec3;
    /// Returns the maximum radius of the primitive. Used for sphere bounding.
    fn max_radius(&self) -> f32;

    /// Dynamically clone type erased collider.
    fn dyn_clone(&self) -> Box<dyn CollisionPrimitive + Send + Sync>;
}

pub trait RayIntersect: CollisionPrimitive + Sized {
    // Returns true if the shape intersects the ray
    fn check_intersect(&self, transform: &TransformMatrix, ray: &Ray) -> bool;
    // Returns the intersection point of a ray onto shape
    fn intersect(&self, transform: &TransformMatrix, ray: &Ray) -> Option<Contact> {
        ray.intersects(self, transform)
    }
}
