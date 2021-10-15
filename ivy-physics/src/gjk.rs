use ultraviolet::{Bivec3, Mat3, Mat4, Vec3};

use crate::collision::{minkowski_diff, CollisionPrimitive, TOLERANCE};

/// Gets the normal of a direction vector with a reference point. Normal will
/// face the same direciton as reference
fn triple_prod(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    a.cross(b).cross(c).normalized()
}

#[derive(Debug)]
pub enum Simplex {
    Point([Vec3; 1]),
    Line([Vec3; 2]),
    Triangle([Vec3; 3]),
    Tetrahedron([Vec3; 4]),
}

impl Simplex {
    // Returns the next simplex that better encloses the origin.
    // Returns None if the origin is enclosed in the tetrahedron.
    #[inline]
    pub fn next(&mut self) -> Option<Vec3> {
        match *self {
            Self::Point([a]) => Some(-a),
            Self::Line([a, b]) => {
                eprintln!("Line case");
                let ab = b - a;
                let a0 = -a;

                if ab.dot(a0) > TOLERANCE {
                    Some(triple_prod(ab, a0, ab))
                } else {
                    *self = Self::Point([a]);
                    Some(a0)
                }
            }
            Simplex::Triangle([a, b, c]) => {
                eprintln!("Triangle case");
                let ab = b - a;
                let ac = c - a;
                let a0 = -a;

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
                eprintln!("Tetrahedron case");
                let ab = b - a;
                let ac = c - a;
                let ad = d - a;
                let a0 = -a;

                let abc = ab.cross(ac);
                let acd = ac.cross(ad);
                let adb = ad.cross(ab);

                if abc.dot(a0) > 0.0 {
                    eprintln!("abc");
                    *self = Self::Triangle([a, b, c]);
                    self.next()
                } else if acd.dot(a0) > 0.0 {
                    eprintln!("acd");
                    *self = Self::Triangle([a, c, d]);
                    self.next()
                } else if adb.dot(a0) > 0.0 {
                    eprintln!("adb");
                    *self = Self::Triangle([a, d, b]);
                    self.next()
                } else {
                    // Collision occurred
                    None
                }
            }
        }
        .map(|val| val.normalized())
    }

    //     const AXES: [Vec3; 3] = [
    //         Vec3 {
    //             x: 1.0,
    //             y: 0.0,
    //             z: 0.0,
    //         },
    //         Vec3 {
    //             x: 0.0,
    //             y: 1.0,
    //             z: 0.0,
    //         },
    //         Vec3 {
    //             x: 0.0,
    //             y: 0.0,
    //             z: 1.0,
    //         },
    //     ];

    //     const CARDINALS: [Vec3; 6] = [
    //         Vec3 {
    //             x: 1.0,
    //             y: 0.0,
    //             z: 0.0,
    //         },
    //         Vec3 {
    //             x: -1.0,
    //             y: 0.0,
    //             z: 0.0,
    //         },
    //         Vec3 {
    //             x: 0.0,
    //             y: 1.0,
    //             z: 0.0,
    //         },
    //         Vec3 {
    //             x: 0.0,
    //             y: -1.0,
    //             z: 0.0,
    //         },
    //         Vec3 {
    //             x: 0.0,
    //             y: 0.0,
    //             z: 1.0,
    //         },
    //         Vec3 {
    //             x: 0.0,
    //             y: 0.0,
    //             z: -1.0,
    //         },
    //     ];

    //     /// Forces the simplex into a tetrahedron by expansion
    //     pub fn force_tetrahedron<A: CollisionPrimitive, B: CollisionPrimitive>(
    //         &mut self,
    //         a_transform: &Mat4,
    //         b_transform: &Mat4,
    //         a_transform_inv: &Mat4,
    //         b_transform_inv: &Mat4,
    //         a_coll: &A,
    //         b_coll: &B,
    //     ) {
    //         match self {
    //             Simplex::Point([a]) => {
    //                 for dir in Self::CARDINALS {
    //                     let p = minkowski_diff(
    //                         a_transform,
    //                         b_transform,
    //                         a_transform_inv,
    //                         b_transform_inv,
    //                         a_coll,
    //                         b_coll,
    //                         dir,
    //                     );

    //                     if (*a - p).mag_sq() > TOLERANCE {
    //                         self.push(p);
    //                         return self.force_tetrahedron(
    //                             a_transform,
    //                             b_transform,
    //                             a_transform_inv,
    //                             b_transform_inv,
    //                             a_coll,
    //                             b_coll,
    //                         );
    //                     }
    //                 }
    //             }
    //             Simplex::Line([a, b]) => {
    //                 let line = *b - *a;
    //                 let min_component = if line.x < line.y {
    //                     if line.x < line.z {
    //                         0
    //                     } else {
    //                         2
    //                     }
    //                 } else if line.y < line.z {
    //                     1
    //                 } else {
    //                     2
    //                 };

    //                 eprintln!("Min: {}; {:?}", min_component, line);

    //                 let mut dir = line.cross(Self::AXES[min_component]).normalized();

    //                 let rot = Mat3::from_angle_plane(
    //                     -std::f32::consts::PI / 3.0,
    //                     Bivec3::from_normalized_axis(line),
    //                 );

    //                 for _ in 0..6 {
    //                     let p = minkowski_diff(
    //                         a_transform,
    //                         b_transform,
    //                         a_transform_inv,
    //                         b_transform_inv,
    //                         a_coll,
    //                         b_coll,
    //                         dir,
    //                     );

    //                     if p.mag_sq() > TOLERANCE {
    //                         self.push(p);
    //                         return self.force_tetrahedron(
    //                             a_transform,
    //                             b_transform,
    //                             a_transform_inv,
    //                             b_transform_inv,
    //                             a_coll,
    //                             b_coll,
    //                         );
    //                     }

    //                     dir = (rot * dir).normalized();
    //                 }
    //                 unreachable!()
    //             }
    //             Simplex::Triangle([a, b, c]) => {
    //                 let ab = *b - *a;
    //                 let ac = *c - *a;

    //                 let dir = ac.cross(ab).normalized();

    //                 let p = minkowski_diff(
    //                     a_transform,
    //                     b_transform,
    //                     a_transform_inv,
    //                     b_transform_inv,
    //                     a_coll,
    //                     b_coll,
    //                     dir,
    //                 );

    //                 // Try again in the opposite direction
    //                 if p.mag_sq() < TOLERANCE {
    //                     let dir = -dir;
    //                     let p = minkowski_diff(
    //                         a_transform,
    //                         b_transform,
    //                         a_transform_inv,
    //                         b_transform_inv,
    //                         a_coll,
    //                         b_coll,
    //                         dir,
    //                     );
    //                     self.push(p)
    //                 } else {
    //                     self.push(p);
    //                 }
    //             }
    //             Simplex::Tetrahedron(_) => {}
    //         }
    //     }

    /// Add a point to the simplex.
    /// Note: Resulting simplex can not contain more than 4 points
    #[inline]
    pub fn push(&mut self, p: Vec3) {
        match self {
            Simplex::Point([a]) => *self = Simplex::Line([p, *a]),
            Simplex::Line([a, b]) => *self = Simplex::Triangle([p, *a, *b]),
            Simplex::Triangle([a, b, c]) => *self = Simplex::Tetrahedron([p, *a, *b, *c]),
            Simplex::Tetrahedron(_) => unreachable!(),
        }
    }

    pub fn points(&self) -> &[Vec3] {
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
pub fn check_intersect<A: CollisionPrimitive, B: CollisionPrimitive>(
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
        assert!((dir.mag() - 1.0).abs() < 0.1);
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
            eprintln!("Collision failed with: {}", p.dot(dir));
            return (false, simplex);
        }

        simplex.push(p);
    }

    // Collision found
    return (true, simplex);
}
