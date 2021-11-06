use std::ops::Index;

use ultraviolet::Vec3;

use crate::util::{project_plane, triple_prod, SupportPoint};

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
            Self::Point([a]) => Some(-a.support),
            Self::Line([a, b]) => {
                let ab = b.support - a.support;
                let a0 = -a.support;

                if ab.dot(a0) > 0.0 {
                    Some(triple_prod(ab, a0, ab))
                } else {
                    *self = Self::Point([a]);
                    Some(a0)
                }
            }
            Simplex::Triangle([a, b, c]) => {
                let ab = b.support - a.support;
                let ac = c.support - a.support;
                let a0 = -a.support;

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
                let ab = b.support - a.support;
                let ac = c.support - a.support;
                let ad = d.support - a.support;
                let a0 = -a.support;

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

    #[inline]
    pub fn next_flat(&mut self, normal: Vec3) -> Option<Vec3> {
        match *self {
            Self::Point([a]) => Some(project_plane(-a.support, normal)),
            Self::Line([a, b]) => {
                let ab = b.support - a.support;
                let a0 = -a.support;

                // let perp = ab.cross(normal).normalized();
                let perp = ab.cross(normal);

                let perp = perp * perp.dot(a0).signum();

                // // Project onto the plane of the vector
                // let perp = (perp - normal * perp.dot(normal)).normalized();
                // dbg!(a.pos, b.pos, perp, normal, perp.dot(normal));
                Some(perp)
            }
            Self::Triangle([a, b, c]) => {
                let ab = b.support - a.support;
                let ac = c.support - a.support;
                let a0 = -a.support;

                let ab = project_plane(ab, normal);
                let ac = project_plane(ac, normal);
                let a0 = project_plane(a0, normal);

                let perp = triple_prod(ac, ab, ab);

                if perp.dot(a0) > 0.0 {
                    *self = Simplex::Line([a, b]);
                    return Some(perp);
                }

                let perp = triple_prod(ab, ac, ac);

                if perp.dot(a0) > 0.0 {
                    *self = Simplex::Line([a, c]);
                    return Some(perp.normalized());
                }

                None
            }
            Simplex::Tetrahedron(_) => unreachable!(),
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

    pub fn iter<'a>(&'a self) -> std::slice::Iter<'a, SupportPoint> {
        self.into_iter()
    }

    pub fn len(&self) -> usize {
        match self {
            Simplex::Point(_) => 1,
            Simplex::Line(_) => 2,
            Simplex::Triangle(_) => 3,
            Simplex::Tetrahedron(_) => 4,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Index<usize> for Simplex {
    type Output = SupportPoint;

    fn index(&self, index: usize) -> &Self::Output {
        &self.points()[index]
    }
}

impl<'a> IntoIterator for &'a Simplex {
    type Item = &'a SupportPoint;

    type IntoIter = std::slice::Iter<'a, SupportPoint>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            Simplex::Point(val) => val.into_iter(),
            Simplex::Line(val) => val.into_iter(),
            Simplex::Triangle(val) => val.into_iter(),
            Simplex::Tetrahedron(val) => val.into_iter(),
        }
    }
}
