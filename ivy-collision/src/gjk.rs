use ultraviolet::{Mat4, Vec3};

use crate::{util::minkowski_diff, util::SupportPoint, CollisionPrimitive};

/// Gets the normal of a direction vector with a reference point. Normal will
/// face the same direciton as reference
fn triple_prod(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    a.cross(b).cross(c).normalized()
}

#[derive(Debug)]
pub enum Simplex {
    Point([SupportPoint; 1]),
    Line([SupportPoint; 2]),
    Triangle([SupportPoint; 3]),
    Tetrahedron([SupportPoint; 4]),
}

impl Simplex {
    // Returns the next simplex that better encloses the origin.
    // Returns None if the origin is enclosed in the tetrahedron.
    #[inline]
    pub fn next(&mut self) -> Option<Vec3> {
        match *self {
            Self::Point([a]) => Some(-a.pos),
            Self::Line([a, b]) => {
                let ab = b.pos - a.pos;
                let a0 = -a.pos;

                if ab.dot(a0) > 0.0 {
                    Some(triple_prod(ab, a0, ab))
                } else {
                    *self = Self::Point([a]);
                    Some(a0)
                }
            }
            Simplex::Triangle([a, b, c]) => {
                let ab = b.pos - a.pos;
                let ac = c.pos - a.pos;
                let a0 = -a.pos;

                let abc = ab.cross(ac);

                // Outside ac face
                if abc.cross(ac).dot(a0) > 0.0 {
                    // Outside but along ac
                    if ac.dot(a0) > 0.0 {
                        *self = Self::Line([a, c]);
                        Some(ac.cross(a0).cross(ac))
                    }
                    // Behind a
                    else {
                        *self = Self::Line([a, b]);
                        self.next()
                    }
                } else if ab.cross(abc).dot(a0) > 0.0 {
                    *self = Self::Line([a, b]);
                    self.next()
                } else if abc.dot(a0) > 0.0 {
                    Some(abc)
                } else {
                    *self = Self::Triangle([a, c, b]);
                    Some(-abc)
                }
            }
            Simplex::Tetrahedron([a, b, c, d]) => {
                let ab = b.pos - a.pos;
                let ac = c.pos - a.pos;
                let ad = d.pos - a.pos;
                let a0 = -a.pos;

                let abc = ab.cross(ac);
                let acd = ac.cross(ad);
                let adb = ad.cross(ab);

                if abc.dot(a0) > 0.0 {
                    *self = Self::Triangle([a, b, c]);
                    self.next()
                } else if acd.dot(a0) > 0.0 {
                    *self = Self::Triangle([a, c, d]);
                    self.next()
                } else if adb.dot(a0) > 0.0 {
                    *self = Self::Triangle([a, d, b]);
                    self.next()
                } else {
                    // Collision occurred
                    None
                }
            }
        }
    }

    /// Add a point to the simplex.
    /// Note: Resulting simplex can not contain more than 4 points
    #[inline]
    pub fn push(&mut self, p: SupportPoint) {
        match self {
            Simplex::Point([a]) => *self = Simplex::Line([p, *a]),
            Simplex::Line([a, b]) => *self = Simplex::Triangle([p, *a, *b]),
            Simplex::Triangle([a, b, c]) => *self = Simplex::Tetrahedron([p, *a, *b, *c]),
            Simplex::Tetrahedron(_) => unreachable!(),
        }
    }

    pub fn points(&self) -> &[SupportPoint] {
        match self {
            Simplex::Point(val) => val,
            Simplex::Line(val) => val,
            Simplex::Triangle(val) => val,
            Simplex::Tetrahedron(val) => val,
        }
    }
}

/// Performs a gjk intersection test.
/// Returns true if the shapes intersect.
pub fn gjk<A: CollisionPrimitive, B: CollisionPrimitive>(
    a_transform: &Mat4,
    b_transform: &Mat4,
    a_transform_inv: &Mat4,
    b_transform_inv: &Mat4,
    a_coll: &A,
    b_coll: &B,
) -> (bool, Simplex) {
    let a_pos = a_transform.extract_translation();
    let b_pos = b_transform.extract_translation();

    // Get first support function in direction of separation
    let dir = (a_pos - b_pos).normalized();
    let a = minkowski_diff(
        &a_transform,
        &b_transform,
        &a_transform_inv,
        &b_transform_inv,
        a_coll,
        b_coll,
        dir,
    );

    let mut simplex = Simplex::Point([a]);

    while let Some(dir) = simplex.next() {
        let dir = dir.normalized();

        assert!((dir.mag() - 1.0 < 0.0001));
        // Get the next simplex
        let p = minkowski_diff(
            &a_transform,
            &b_transform,
            &a_transform_inv,
            &b_transform_inv,
            a_coll,
            b_coll,
            dir,
        );

        // New point was not past the origin
        // No collision
        if p.dot(dir) < 0.0 {
            return (false, simplex);
        }

        // p.pos += p.normalized() * 1.0;

        simplex.push(p);
    }

    // Collision found
    return (true, simplex);
}
