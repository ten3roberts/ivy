use std::ops::Deref;

use ivy_base::{Position, Scale, TransformMatrix};
use ultraviolet::Vec3;

use crate::{CollisionPrimitive, Ray, RayIntersect};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cube {
    pub half_extents: Vec3,
}

impl Cube {
    pub fn new(half_extents: Vec3) -> Self {
        Self { half_extents }
    }

    pub fn uniform(half_extent: f32) -> Self {
        Self {
            half_extents: Vec3::new(half_extent, half_extent, half_extent),
        }
    }

    /// Performs ray intersection testing by assuming the cube is axis aligned
    /// and has a scale of 1.0
    pub fn check_aabb_intersect(&self, position: Position, scale: Scale, ray: &Ray) -> bool {
        let dir = ray.dir();
        let inv_dir = Vec3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z);

        let origin = ray.origin - position;

        let t1 = (-self.half_extents * *scale - *origin) * inv_dir;
        let t2 = (self.half_extents * *scale - *origin) * inv_dir;
        let tmin = t1.min_by_component(t2);
        let tmax = t1.max_by_component(t2);

        tmin.component_max() <= tmax.component_min()
    }
}

impl Deref for Cube {
    type Target = Vec3;

    fn deref(&self) -> &Self::Target {
        &self.half_extents
    }
}

impl CollisionPrimitive for Cube {
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
        self.half_extents.mag()
    }

    fn dyn_clone(&self) -> Box<dyn CollisionPrimitive + Send + Sync> {
        Box::new(self.clone())
    }
}

impl RayIntersect for Cube {
    // https://www.jcgt.org/published/0007/03/04/paper-lowres.pdf
    fn check_intersect(&self, transform: &TransformMatrix, ray: &Ray) -> bool {
        let inv = transform.inversed();
        let dir = inv.transform_vec3(ray.dir()).normalized();
        let inv_dir = Vec3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z);

        let origin = inv.transform_point3(*ray.origin);

        let t1 = (-self.half_extents - origin) * inv_dir;
        let t2 = (self.half_extents - origin) * inv_dir;
        let tmin = t1.min_by_component(t2);
        let tmax = t1.max_by_component(t2);

        if tmax.component_min() < 0.0 {
            return false;
        }

        tmin.component_max() <= tmax.component_min()
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
    pub fn overlaps(&self, origin: Position, other: &Self, other_origin: Position) -> bool {
        let total_radii = self.radius + other.radius;

        (origin - other_origin).mag_sq() < total_radii * total_radii
    }

    /// Creates a bounding sphere fully enclosign a primitive
    #[inline]
    pub fn enclose<T: CollisionPrimitive>(collider: &T, scale: Scale) -> Self {
        Self {
            radius: collider.max_radius() * scale.component_max(),
        }
    }

    /// Checks an axis aligned perfect sphere ray intersection
    pub fn check_aa_intersect(&self, pos: Position, ray: &Ray) -> bool {
        let dir = ray.dir();
        let origin = *ray.origin - *pos;

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

impl CollisionPrimitive for Sphere {
    fn support(&self, dir: Vec3) -> Vec3 {
        self.radius * dir
    }

    fn max_radius(&self) -> f32 {
        self.radius
    }

    fn dyn_clone(&self) -> Box<dyn CollisionPrimitive + Send + Sync> {
        Box::new(self.clone())
    }
}

impl RayIntersect for Sphere {
    // https://gist.github.com/wwwtyro/beecc31d65d1004f5a9d
    fn check_intersect(&self, transform: &TransformMatrix, ray: &Ray) -> bool {
        let inv = transform.inversed();
        let dir = inv.transform_vec3(ray.dir()).normalized();
        let origin = inv.transform_point3(*ray.origin);

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

impl CollisionPrimitive for Capsule {
    fn support(&self, dir: Vec3) -> Vec3 {
        let mut result = Vec3::zero();
        result.y = dir.y.signum() * self.half_height;
        result + dir * self.radius
    }

    fn max_radius(&self) -> f32 {
        self.half_height + self.radius
    }

    fn dyn_clone(&self) -> Box<dyn CollisionPrimitive + Send + Sync> {
        Box::new(self.clone())
    }
}
