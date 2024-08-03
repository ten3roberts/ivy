use glam::Vec3;
use ivy_core::{Cube, DrawGizmos, GizmosSection};

use crate::Ray;

/// Represents an axis aligned bounding box
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct BoundingBox {
    pub origin: Vec3,
    pub extents: Vec3,
}

impl BoundingBox {
    pub fn new(half_extents: Vec3, origin: Vec3) -> Self {
        Self {
            origin,
            extents: half_extents,
        }
    }

    /// Creates a new boundning box from bottom left back corner to the top
    /// right front corner.
    pub fn from_corners(l: Vec3, r: Vec3) -> Self {
        let origin = (l + r) * 0.5;

        let half_extents = (r - l) * 0.5;

        Self::new(half_extents, origin.into())
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

    pub fn into_corners(&self) -> (Vec3, Vec3) {
        let l = Vec3::new(self.neg_x(), self.neg_y(), self.neg_z());
        let r = Vec3::new(self.x(), self.y(), self.z());

        (l, r)
    }

    #[inline]
    pub fn x(&self) -> f32 {
        self.extents.x + self.origin.x
    }

    #[inline]
    pub fn neg_x(&self) -> f32 {
        -self.extents.x + self.origin.x
    }

    #[inline]
    pub fn y(&self) -> f32 {
        self.extents.y + self.origin.y
    }

    #[inline]
    pub fn neg_y(&self) -> f32 {
        -self.extents.y + self.origin.y
    }

    #[inline]
    pub fn z(&self) -> f32 {
        self.extents.z + self.origin.z
    }

    #[inline]
    pub fn neg_z(&self) -> f32 {
        -self.extents.z + self.origin.z
    }

    pub fn contains(&self, other: BoundingBox) -> bool {
        self.x() >= other.x()
            && self.neg_x() <= other.neg_x()
            && self.y() >= other.y()
            && self.neg_y() <= other.neg_y()
            && self.z() >= other.z()
            && self.neg_z() <= other.neg_z()
    }

    /// Performs ray intersection testing by assuming the cube is axis aligned
    /// and has a scale of 1.0
    pub fn check_ray(&self, ray: &Ray) -> bool {
        let dir = ray.dir();
        let inv_dir = Vec3::new(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z);

        let origin = ray.origin - self.origin;

        let t1 = (-self.extents - origin) * inv_dir;
        let t2 = (self.extents - origin) * inv_dir;
        let tmin = t1.min(t2);
        let tmax = t1.max(t2);

        tmin.max_element() <= tmax.min_element()
    }

    pub fn overlaps(&self, other: Self) -> bool {
        self.x() >= other.neg_x()
            && self.neg_x() <= other.x()
            && self.y() >= other.neg_y()
            && self.neg_y() <= other.y()
            && self.z() >= other.neg_z()
            && self.neg_z() <= other.z()
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
        let (l0, r0) = self.into_corners();
        let (l1, r1) = other.into_corners();

        let l = l0.min(l1);
        let r = r0.max(r1);

        Self::from_corners(l, r)
    }

    /// Returns a new bounding box with a margin
    pub fn margin(&self, margin: f32) -> BoundingBox {
        BoundingBox {
            origin: self.origin,
            extents: self.extents + Vec3::new(0.5, 0.5, 0.5) * margin,
        }
    }

    /// Returns a new bounding box with a margin that is proprtional to the
    /// extents.
    /// If `margin` is less than 1, the bounding box may not contain the original
    /// object
    pub fn rel_margin(&self, margin: f32) -> BoundingBox {
        BoundingBox {
            origin: self.origin,
            extents: self.extents * margin,
        }
    }

    pub(crate) fn expand(&self, amount: Vec3) -> BoundingBox {
        let extents = self.extents + amount.abs();
        let origin = self.origin + amount * 0.5;

        BoundingBox { origin, extents }
    }
}

impl DrawGizmos for BoundingBox {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        gizmos.draw(Cube {
            origin: self.origin,
            half_extents: self.extents,
            ..Default::default()
        });
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

        assert_eq!(bounds.extents, vec3(0.5, 1.5, 0.5));
        assert_eq!(bounds.origin, vec3(-0.5, 0.5, -0.5));
        assert_eq!(bounds.into_corners(), (l, r));

        let smaller = BoundingBox::new(vec3(0.5, 0.5, 0.5), Vec3::ZERO);

        dbg!(bounds.y(), smaller.y());

        assert!(bounds.overlaps(smaller));
        assert!(!bounds.contains(smaller));

        let larger = bounds.merge(smaller);

        assert!(larger.contains(bounds));
        assert!(larger.contains(smaller));
    }
}
