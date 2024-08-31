use core::f32;

use glam::{vec3, Vec3};
use ivy_core::gizmos::{Cube, DrawGizmos, GizmosSection};
use ordered_float::NotNan;
use palette::num::{Abs, Signum};

use crate::{util::TOLERANCE, Ray, Shape};

/// Represents an axis aligned bounding box
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    pub min: Vec3,
    pub max: Vec3,
}

impl BoundingBox {
    pub fn new(half_extents: Vec3, origin: Vec3) -> Self {
        Self {
            min: origin - half_extents,
            max: origin + half_extents,
        }
    }

    /// Creates a new boundning box from bottom left back corner to the top
    /// right front corner.
    pub fn from_corners(min: Vec3, max: Vec3) -> Self {
        Self { min, max }
    }

    pub fn from_points(points: impl Iterator<Item = Vec3>) -> Self {
        let mut l = Vec3::new(f32::MAX, f32::MAX, f32::MAX);
        let mut r = Vec3::new(f32::MIN, f32::MIN, f32::MIN);

        points.for_each(|val| {
            l = l.min(val);
            r = r.max(val);
        });

        BoundingBox::from_corners(l, r)
    }

    #[inline]
    pub fn x(&self) -> f32 {
        self.max.x
    }

    #[inline]
    pub fn neg_x(&self) -> f32 {
        self.min.x
    }

    #[inline]
    pub fn y(&self) -> f32 {
        self.max.y
    }

    #[inline]
    pub fn neg_y(&self) -> f32 {
        self.min.y
    }

    #[inline]
    pub fn z(&self) -> f32 {
        self.max.z
    }

    #[inline]
    pub fn neg_z(&self) -> f32 {
        -self.min.z
    }

    pub fn contains(&self, other: BoundingBox) -> bool {
        self.min.x <= other.min.x
            && self.min.y <= other.min.y
            && self.min.z <= other.min.z
            && self.max.x >= other.max.x
            && self.max.y >= other.max.y
            && self.max.z >= other.max.z
    }

    /// Performs ray intersection testing by assuming the cube is axis aligned
    /// and has a scale of 1.0
    pub fn check_ray(&self, ray: &Ray) -> bool {
        let dir = ray.dir();
        let inv_dir = Vec3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z);

        let origin = ray.origin - (self.min + self.max) / 2.0;

        let extents = self.max - self.min;

        let t1 = (-extents - origin) * inv_dir;
        let t2 = (extents - origin) * inv_dir;
        let tmin = t1.min(t2);
        let tmax = t1.max(t2);

        tmin.max_element() <= tmax.min_element()
    }

    pub fn overlaps(&self, other: Self) -> bool {
        self.max.x >= other.min.x
            && self.min.x <= other.max.x
            && self.max.y >= other.min.y
            && self.min.y <= other.max.y
            && self.max.z >= other.min.z
            && self.min.z <= other.max.z
    }

    pub fn contains_point(&self, point: Vec3) -> bool {
        self.x() >= point.x
            && self.neg_x() <= point.x
            && self.y() >= point.y
            && self.neg_y() <= point.y
            && self.z() >= point.z
            && self.neg_z() <= point.z
    }

    /// Creates a new bounding box encompassing both
    pub fn merge(&self, other: Self) -> Self {
        let l = self.min.min(other.min);
        let r = self.max.max(other.max);

        Self::from_corners(l, r)
    }

    /// Returns a new bounding box with a margin
    pub fn margin(&self, margin: f32) -> BoundingBox {
        BoundingBox {
            min: self.min - margin,
            max: self.max + margin,
        }
    }

    /// Returns a new bounding box with a margin that is proprtional to the
    /// extents.
    /// If `margin` is less than 1, the bounding box may not contain the original
    /// object
    pub fn rel_margin(&self, margin: f32) -> BoundingBox {
        let size = self.max - self.min;
        BoundingBox {
            min: self.min - size * margin,
            max: self.max + size * margin,
        }
    }

    pub(crate) fn expand(&self, amount: Vec3) -> BoundingBox {
        BoundingBox {
            min: self.min - amount,
            max: self.max + amount,
        }
    }

    pub fn midpoint(&self) -> Vec3 {
        (self.min + self.max) / 2.0
    }

    pub fn size(&self) -> Vec3 {
        self.max - self.min
    }
}

impl DrawGizmos for BoundingBox {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        gizmos.draw(Cube {
            min: self.min,
            max: self.max,
            ..Default::default()
        });
    }
}

impl Shape for BoundingBox {
    fn support(&self, dir: Vec3) -> Vec3 {
        let x = if dir.x > 0.0 { self.max.x } else { self.min.x };
        let y = if dir.y > 0.0 { self.max.y } else { self.min.y };
        let z = if dir.z > 0.0 { self.max.z } else { self.min.z };

        vec3(x, y, z)
    }

    fn surface_contour(&self, dir: Vec3, points: &mut Vec<Vec3>) {
        const TOLERANCE: f32 = 0.1;

        assert!(dir.is_normalized());
        let corners = [
            vec3(self.min.x, self.min.y, self.min.z),
            vec3(self.min.x, self.min.y, self.max.z),
            vec3(self.min.x, self.max.y, self.max.z),
            vec3(self.min.x, self.max.y, self.min.z),
            vec3(self.max.x, self.min.y, self.min.z),
            vec3(self.max.x, self.min.y, self.max.z),
            vec3(self.max.x, self.max.y, self.max.z),
            vec3(self.max.x, self.max.y, self.min.z),
        ];

        let support_dist = self.support(dir).dot(dir);

        points.extend(corners.iter().filter(|v| {
            let dist = (support_dist - v.dot(dir)).abs();
            // tracing::info!(
            //     ?support_dist,
            //     dot = v.dot(dir),
            //     pass = dist < TOLERANCE,
            //     "point"
            // );
            dist < TOLERANCE
        }));

        // let extreme = |v| {
        //     if v > 0.0 {
        //         1.0
        //     } else if v < 0.0 {
        //         -1.0
        //     } else {
        //         0.0
        //     }
        // };

        // let dir = vec3(extreme(dir.x), extreme(dir.y), extreme(dir.z));

        let tan = if dir.dot(Vec3::X).abs() > 1.0 - TOLERANCE {
            Vec3::Y * dir.dot(Vec3::X).signum()
        } else {
            dir.cross(Vec3::X).normalize()
        };

        let bitan = tan.cross(dir).normalize();
        assert!(points.len() <= 4, "Too many points: {points:?}");
        // assert!([1, 2, 4].contains(&points.len()));

        let midpoint = self.midpoint();

        // sort points by the angle to ensure correct winding
        let reference_point = points[0];
        if points.len() == 4 {
            points.sort_by_key(|&v| {
                let v = (v - midpoint).normalize_or_zero();
                let x = v.dot(tan);
                let y = v.dot(bitan);

                NotNan::new(x.atan2(y)).unwrap()
            });
        }
    }

    fn max_radius(&self) -> f32 {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use glam::{vec3, Vec3};

    use crate::BoundingBox;

    #[test]
    fn bounding_box() {
        let l = Vec3::new(-1.0, -1.0, -1.0);
        let r = Vec3::new(0.0, 2.0, 0.0);

        let bounds = BoundingBox::from_corners(l, r);

        assert_eq!(bounds.size(), vec3(0.5, 1.5, 0.5));
        assert_eq!(bounds.midpoint(), vec3(-0.5, 0.5, -0.5));
        assert_eq!((bounds.min, bounds.max), (l, r));

        let smaller = BoundingBox::new(vec3(0.5, 0.5, 0.5), Vec3::ZERO);

        dbg!(bounds.y(), smaller.y());

        assert!(bounds.overlaps(smaller));
        assert!(!bounds.contains(smaller));

        let larger = bounds.merge(smaller);

        assert!(larger.contains(bounds));
        assert!(larger.contains(smaller));
    }
}
