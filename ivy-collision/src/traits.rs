use std::fmt::Debug;

use glam::{Mat4, Vec3};

use crate::{BoundingBox, Intersection, Ray};

pub trait Shape: Debug {
    /// Returns the furtherst vertex in `dir`.
    /// Direction is given in collider/model space.
    fn support(&self, dir: Vec3) -> Vec3;

    /// Returns the maximum radius of the primitive. Used for sphere bounding.
    fn max_radius(&self) -> f32;

    fn surface_contour(&self, dir: Vec3, points: &mut Vec<Vec3>);

    /// Returns an axis aligned bounding box enclosing the shape at the current
    /// rotation and scale
    fn bounding_box(&self, transform: Mat4) -> BoundingBox
    where
        Self: Sized,
    {
        let shape = TransformedShape::new(self, transform);
        let lx = shape.support(-Vec3::X).x;
        let ly = shape.support(-Vec3::Y).y;
        let lz = shape.support(-Vec3::Z).z;

        let rx = shape.support(Vec3::X).x;
        let ry = shape.support(Vec3::Y).y;
        let rz = shape.support(Vec3::Z).z;

        BoundingBox::from_corners(Vec3::new(lx, ly, lz), Vec3::new(rx, ry, rz))
    }
}

impl<'a, T> Shape for &'a T
where
    T: Shape,
{
    fn support(&self, dir: Vec3) -> Vec3 {
        (*self).support(dir)
    }

    fn surface_contour(&self, dir: Vec3, points: &mut Vec<Vec3>) {
        (*self).surface_contour(dir, points)
    }

    fn max_radius(&self) -> f32 {
        (*self).max_radius()
    }
}

pub trait RayIntersect: Shape + Sized {
    // Returns true if the shape intersects the ray
    fn check_intersect(&self, transform: &Mat4, ray: &Ray) -> bool;
    // Returns the intersection point of a ray onto shape
    fn intersect(&self, transform: &Mat4, ray: &Ray) -> Option<Intersection> {
        ray.intersects(self, transform)
    }
}

#[derive(Clone)]
pub struct TransformedShape<T> {
    shape: T,
    transform: Mat4,
    inv_transform: Mat4,
}

impl<T> TransformedShape<T> {
    pub fn new(shape: T, transform: Mat4) -> Self {
        Self {
            shape,
            transform,
            inv_transform: transform.inverse(),
        }
    }
}

impl<T: Debug> Debug for TransformedShape<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let (scale, rotation, translation) = self.transform.to_scale_rotation_translation();
        f.debug_struct("TransformedShape")
            .field("shape", &self.shape)
            .field("translation", &translation)
            .field("rotation", &rotation)
            .field("scale", &scale)
            .finish()
    }
}

impl<T: Shape> Shape for TransformedShape<T> {
    fn support(&self, dir: Vec3) -> Vec3 {
        let local_support = self
            .shape
            .support(self.inv_transform.transform_vector3(dir).normalize());
        self.transform.transform_point3(local_support)
    }

    fn surface_contour(&self, dir: Vec3, points: &mut Vec<Vec3>) {
        self.shape.surface_contour(
            self.inv_transform.transform_vector3(dir).normalize(),
            points,
        );

        for point in points {
            *point = self.transform.transform_point3(*point)
        }
    }

    fn max_radius(&self) -> f32 {
        self.shape.max_radius()
    }
}
