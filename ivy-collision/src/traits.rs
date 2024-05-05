use std::fmt::Debug;

use glam::{Mat4, Vec3};

use crate::{util::support, BoundingBox, Contact, Ray};

pub trait CollisionPrimitive: Debug {
    /// Returns the furtherst vertex in `dir`.
    /// Direction is given in collider/model space.
    fn support(&self, dir: Vec3) -> Vec3;
    /// Returns the maximum radius of the primitive. Used for sphere bounding.
    fn max_radius(&self) -> f32;

    /// Dynamically clone type erased collider.
    #[deprecated(note = "Use `Arc` instead")]
    fn dyn_clone(&self) -> Box<dyn CollisionPrimitive + Send + Sync>;

    /// Returns an axis aligned bounding box enclosing the shape at the current
    /// rotation and scale
    fn bounding_box(&self, transform: Mat4) -> BoundingBox
    where
        Self: Sized,
    {
        let inv = transform.inverse();

        let lx = support(&transform, &inv, self, -Vec3::X).x;
        let ly = support(&transform, &inv, self, -Vec3::Y).y;
        let lz = support(&transform, &inv, self, -Vec3::Z).z;

        let rx = support(&transform, &inv, self, Vec3::X).x;
        let ry = support(&transform, &inv, self, Vec3::Y).y;
        let rz = support(&transform, &inv, self, Vec3::Z).z;

        BoundingBox::from_corners(Vec3::new(lx, ly, lz), Vec3::new(rx, ry, rz))
    }
}

pub trait RayIntersect: CollisionPrimitive + Sized {
    // Returns true if the shape intersects the ray
    fn check_intersect(&self, transform: &Mat4, ray: &Ray) -> bool;
    // Returns the intersection point of a ray onto shape
    fn intersect(&self, transform: &Mat4, ray: &Ray) -> Option<Contact> {
        ray.intersects(self, transform)
    }
}
