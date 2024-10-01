use std::ops::Deref;

use glam::{Mat4, Vec2, Vec3};
use ordered_float::Float;
use palette::num::{Abs, Signum};

use crate::{util::TOLERANCE, Ray, RayIntersect, Shape};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cube {
    pub half_extents: Vec3,
}

impl Cube {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self {
            half_extents: Vec3::new(x, y, z),
        }
    }

    pub fn uniform(half_extent: f32) -> Self {
        Self {
            half_extents: Vec3::new(half_extent, half_extent, half_extent),
        }
    }
}

impl Deref for Cube {
    type Target = Vec3;

    fn deref(&self) -> &Self::Target {
        &self.half_extents
    }
}

impl Shape for Cube {
    fn support(&self, dir: Vec3) -> Vec3 {
        let x = if dir.x > 0.0 {
            self.half_extents.x
        } else {
            -self.half_extents.x
        };
        let y = if dir.y > 0.0 {
            self.half_extents.y
        } else {
            -self.half_extents.y
        };
        let z = if dir.z > 0.0 {
            self.half_extents.z
        } else {
            -self.half_extents.z
        };

        Vec3::new(x, y, z)
    }

    fn max_radius(&self) -> f32 {
        // TODO: incorrect, radius is not the best in general
        self.half_extents.max_element()
    }

    fn surface_contour(&self, dir: Vec3, points: &mut Vec<Vec3>) {
        todo!()
    }

    fn center(&self) -> Vec3 {
        todo!()
    }
}

impl RayIntersect for Cube {
    // https://www.jcgt.org/published/0007/03/04/paper-lowres.pdf
    fn check_intersect(&self, transform: &Mat4, ray: &Ray) -> bool {
        let inv = transform.inverse();
        let dir = inv.transform_vector3(ray.dir()).normalize();
        let inv_dir = Vec3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z);

        let origin = inv.transform_point3(ray.origin);

        let t1 = (-self.half_extents - origin) * inv_dir;
        let t2 = (self.half_extents - origin) * inv_dir;
        let tmin = t1.min(t2);
        let tmax = t1.max(t2);

        if tmax.min_element() < 0.0 {
            return false;
        }

        tmin.max_element() <= tmax.min_element()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Sphere {
    pub radius: f32,
}

impl Sphere {
    pub fn new(radius: f32) -> Self {
        Self { radius }
    }

    /// Returns true if two uniform spheres overlap.
    #[inline]
    pub fn overlaps(&self, origin: Vec3, other: &Self, other_origin: Vec3) -> bool {
        let total_radii = self.radius + other.radius;

        (origin - other_origin).length_squared() < total_radii * total_radii
    }

    /// Creates a bounding sphere fully enclosign a primitive
    #[inline]
    pub fn enclose<T: Shape>(collider: &T, scale: Vec3) -> Self {
        Self {
            radius: collider.max_radius() * scale.min_element(),
        }
    }

    /// Checks an axis aligned perfect sphere ray intersection
    pub fn check_aa_intersect(&self, pos: Vec3, ray: &Ray) -> bool {
        let dir = ray.dir();
        let origin = ray.origin - pos;

        let a = dir.dot(dir);

        let b = 2.0 * dir.dot(origin);
        let c = origin.dot(origin) - (self.radius * self.radius);

        let b2 = b * b;

        let dis = b2 - 4.0 * a * c;

        if dis < 0.0 {
            return false;
        }

        (-b - (dis).sqrt() / (2.0 * a)) > -1.0
    }
}

impl Shape for Sphere {
    fn support(&self, dir: Vec3) -> Vec3 {
        self.radius * dir
    }

    fn max_radius(&self) -> f32 {
        self.radius
    }

    fn surface_contour(&self, dir: Vec3, points: &mut Vec<Vec3>) {
        points.push(self.radius * dir)
    }

    fn center(&self) -> Vec3 {
        Vec3::ZERO
    }
}

impl RayIntersect for Sphere {
    // https://gist.github.com/wwwtyro/beecc31d65d1004f5a9d
    fn check_intersect(&self, transform: &Mat4, ray: &Ray) -> bool {
        let inv = transform.inverse();
        let dir = inv.transform_vector3(ray.dir()).normalize();
        let origin = inv.transform_point3(ray.origin);

        let a = dir.dot(dir);

        let b = 2.0 * dir.dot(origin);
        let c = origin.dot(origin) - (self.radius * self.radius);

        let b2 = b * b;

        let dis = b2 - 4.0 * a * c;

        if dis < 0.0 {
            return false;
        }

        (-b - (dis).sqrt() / (2.0 * a)) > -1.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Capsule {
    pub half_height: f32,
    pub radius: f32,
}

impl Capsule {
    pub fn new(half_height: f32, radius: f32) -> Self {
        Self {
            half_height,
            radius,
        }
    }
}

impl Shape for Capsule {
    fn support(&self, dir: Vec3) -> Vec3 {
        assert!(dir.is_normalized());

        dir.y.signum() * self.half_height * Vec3::Y + dir * self.radius

        // let mut result = Vec3::ZERO;
        // Vec3::Y * dir.y.signum() * self.half_height
        //     + dir.reject_from_normalized(Vec3::Y).normalize_or_zero() * self.radius
        // result.y = dir.y.signum() * self.half_height;
        // result + dir * self.radius
    }

    fn max_radius(&self) -> f32 {
        self.half_height + self.radius
    }

    fn surface_contour(&self, dir: Vec3, points: &mut Vec<Vec3>) {
        assert!(dir.is_normalized());
        const TOLERANCE: f32 = 0.01;
        if dir.dot(Vec3::Y).abs() < TOLERANCE {
            let extension = dir.reject_from_normalized(Vec3::Y).normalize_or_zero() * self.radius;

            points.extend([
                extension + Vec3::Y * self.half_height,
                extension - Vec3::Y * self.half_height,
            ])
        } else {
            let p = dir * self.radius + Vec3::Y * self.half_height * dir.y.signum();
            points.push(p);
        }
    }

    fn center(&self) -> Vec3 {
        Vec3::ZERO
    }
}

impl From<Vec3> for Cube {
    fn from(v: Vec3) -> Self {
        Self { half_extents: v }
    }
}
