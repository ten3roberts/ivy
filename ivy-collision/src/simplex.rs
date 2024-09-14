use std::ops::Index;

use glam::{Vec3, Vec3Swizzles};
use ordered_float::Float;
use palette::{named::SIENNA, num::Abs};
use rand::SeedableRng;

use crate::util::{project_plane, triple_prod, SupportPoint, TOLERANCE};

pub(crate) enum SimplexExpansion {
    Direction(Vec3),
    Degenerate,
    Enveloped,
}

#[derive(Debug)]
pub enum Simplex {
    Point([SupportPoint; 1]),
    Line([SupportPoint; 2]),
    Triangle([SupportPoint; 3]),
    Tetrahedron([SupportPoint; 4]),
}

impl Simplex {
    pub fn is_unique(&self) -> bool {
        match self {
            Simplex::Point(_) => true,
            Simplex::Line([a, b]) => a != b,
            Simplex::Triangle([a, b, c]) => a != b && a != c && b != c,
            Simplex::Tetrahedron([a, b, c, d]) => {
                a != b && a != c && a != d && b != c && b != d && c != d
            }
        }
    }

    /// Returns the next direction more likely to enclose origin
    pub(crate) fn next_dir(&mut self) -> SimplexExpansion {
        match *self {
            Self::Point([a]) => SimplexExpansion::Direction(-a.support),
            Self::Line([a, b]) => {
                let ab = b.support - a.support;
                let a0 = -a.support;

                // assert!(ab.length() > 0.0);

                // tracing::info!(?ab, dot=?ab.dot(a0));

                if ab.normalize().dot(a0.normalize()) > 1.0 - TOLERANCE {
                    // Degenerate, pick new direction
                    *self = Self::Point([b]);
                    let new_dir = if ab.normalize().dot(Vec3::X).abs() < 1.0 - TOLERANCE {
                        ab.cross(Vec3::X)
                    } else {
                        ab.cross(Vec3::Y)
                    };
                    // tracing::warn!(%new_dir, "degenerate edge");
                    return SimplexExpansion::Direction(new_dir);
                }

                if ab.dot(a0) > TOLERANCE {
                    SimplexExpansion::Direction(triple_prod(ab, a0, ab))
                } else if ab.dot(a0) < -TOLERANCE {
                    *self = Self::Point([a]);
                    SimplexExpansion::Direction(a0)
                } else {
                    // Degenerate, pick new direction
                    *self = Self::Point([b]);
                    let new_dir = if ab.normalize().dot(Vec3::X).abs() < 1.0 - TOLERANCE {
                        ab.cross(Vec3::X)
                    } else {
                        ab.cross(Vec3::Y)
                    };
                    // tracing::warn!(%new_dir, "degenerate edge");
                    SimplexExpansion::Direction(new_dir)
                }
            }
            Simplex::Triangle([a, b, c]) => {
                let ab = b.support - a.support;
                let ac = c.support - a.support;
                let a0 = -a.support;

                let abc = ab.cross(ac);

                if abc.cross(ac).dot(a0) > 0.0 {
                    // outside ac
                    if ac.dot(a0) > 0.0 {
                        *self = Self::Line([a, c]);
                        SimplexExpansion::Direction(ac.cross(a0).cross(ac))
                    } else if ac.dot(a0) < 0.0 {
                        *self = Self::Line([a, b]);
                        self.next_dir()
                    } else {
                        panic!("");
                    }
                } else if ab.cross(abc).dot(a0) > 0.0 {
                    // outside ab
                    *self = Self::Line([a, b]);
                    self.next_dir()
                } else if abc.dot(a0) > 0.0 {
                    // inside ac, ab, above triangle
                    SimplexExpansion::Direction(abc)
                } else if abc.dot(a0) < 0.0 {
                    // below triangle
                    *self = Self::Triangle([a, c, b]);
                    SimplexExpansion::Direction(-abc)
                } else {
                    // tracing::warn!("degenerate triangle");
                    *self = Self::Line([a, b]);
                    SimplexExpansion::Degenerate
                }

                // // Outside ac face
                // if abc.cross(ac).dot(a0) > TOLERANCE {
                //     // Outside but along ac
                //     if ac.dot(a0) >= TOLERANCE {
                //         *self = Self::Line([a, c]);
                //         SimplexExpansion::Direction(ac.cross(a0).cross(ac))
                //     }
                //     // Behind a
                //     else if ac.dot(a0) < -TOLERANCE {
                //         *self = Self::Line([a, b]);
                //         self.next_dir()
                //     } else {
                //         SimplexExpansion::Degenerate
                //     }
                // } else if ab.cross(abc).dot(a0) > TOLERANCE {
                //     *self = Self::Line([a, b]);
                //     self.next_dir()
                // } else if abc.dot(a0) > TOLERANCE {
                //     tracing::info!(dot = abc.dot(a0), "above triangle");
                //     SimplexExpansion::Direction(abc)
                // } else if abc.dot(a0) < TOLERANCE {
                //     tracing::info!(dot = abc.dot(a0), "below triangle");
                //     *self = Self::Triangle([a, c, b]);
                //     SimplexExpansion::Direction(-abc)
                // } else {
                //     *self = Self::Line([a, b]);
                //     // let next_dir = ab.cross(ac).normalize();
                //     tracing::warn!("degenerate");
                //     // assert!(next_dir.is_finite());
                //     SimplexExpansion::Degenerate
                // }
            }
            Simplex::Tetrahedron([a, b, c, d]) => {
                let ab = b.support - a.support;
                let ac = c.support - a.support;
                let ad = d.support - a.support;
                let a0 = -a.support;

                let abc = ab.cross(ac);
                let acd = ac.cross(ad);
                let adb = ad.cross(ab);

                let abc_dot = abc.dot(a0);
                let acd_dot = acd.dot(a0);
                let adb_dot = adb.dot(a0);

                // tracing::info!(%abc, abc_dot, acd_dot, adb_dot);

                if abc_dot > 0.0 {
                    *self = Self::Triangle([a, b, c]);
                    self.next_dir()
                } else if abc_dot == 0.0 {
                    *self = Self::Line([a, d]);
                    SimplexExpansion::Degenerate
                } else if acd_dot > 0.0 {
                    *self = Self::Triangle([a, c, d]);
                    self.next_dir()
                } else if acd_dot == 0.0 {
                    *self = Self::Line([a, b]);
                    SimplexExpansion::Degenerate
                } else if adb_dot > 0.0 {
                    *self = Self::Triangle([a, d, b]);
                    self.next_dir()
                } else if adb_dot == 0.0 {
                    *self = Self::Line([a, c]);
                    SimplexExpansion::Degenerate
                } else {
                    // Collision occurred
                    SimplexExpansion::Enveloped
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
                    return Some(perp.normalize());
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

    pub fn iter(&self) -> std::slice::Iter<SupportPoint> {
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
            Simplex::Point(val) => val.iter(),
            Simplex::Line(val) => val.iter(),
            Simplex::Triangle(val) => val.iter(),
            Simplex::Tetrahedron(val) => val.iter(),
        }
    }
}
