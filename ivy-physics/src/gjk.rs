use ivy_core::Position;
use ultraviolet::Vec3;

use crate::collision::CollisionPrimitive;

/// Gets the normal of a direction vector with a reference point. Normal will
/// face the same direciton as reference
fn triple_prod(a: Vec3, b: Vec3, c: Vec3) -> Vec3 {
    a.cross(b).cross(c).normalized()
}

#[derive(Debug)]
pub enum Simplex {
    Point(Vec3),
    Line(Vec3, Vec3),
    Triangle(Vec3, Vec3, Vec3),
    Tetrahedron(Vec3, Vec3, Vec3, Vec3),
}

impl Simplex {
    // Returns the next simplex that better encloses the origin.
    // Returns None if the origin is enclosed in the tetrahedron.
    pub fn next(&mut self) -> Option<Vec3> {
        match *self {
            Self::Point(a) => Some(-a),
            Self::Line(a, b) => {
                eprintln!("Line");
                let ab = b - a;
                let a0 = -a;

                if ab.dot(a0) > 0.0 {
                    Some(triple_prod(ab, a0, ab))
                } else {
                    *self = Self::Point(a);
                    Some(a0)
                }
            }
            Simplex::Triangle(a, b, c) => {
                eprintln!("Triangle");
                let ab = b - a;
                let ac = c - a;
                let a0 = -a;

                let abc = ab.cross(ac);

                // Outside ac face
                if abc.cross(ac).dot(a0) > 0.0 {
                    // Outside but along ac
                    if ac.dot(a0) > 0.0 {
                        *self = Self::Line(a, c);
                        Some(ac.cross(a0).cross(ac))
                    }
                    // Behind a
                    else {
                        *self = Self::Line(a, b);
                        self.next()
                    }
                } else if ab.cross(abc).dot(a0) > 0.0 {
                    *self = Self::Line(a, b);
                    self.next()
                } else if abc.dot(a0) > 0.0 {
                    Some(abc)
                } else {
                    *self = Self::Triangle(a, c, b);
                    Some(-abc)
                }
            }
            Simplex::Tetrahedron(a, b, c, d) => {
                eprintln!("Tetrahedron");
                let ab = b - a;
                let ac = c - a;
                let ad = d - a;
                let a0 = -a;

                let abc = ab.cross(ac);
                let acd = ac.cross(ad);
                let adb = ad.cross(ab);

                if abc.dot(a0) > 0.0 {
                    *self = Self::Triangle(a, b, c);
                    self.next()
                } else if acd.dot(a0) > 0.0 {
                    *self = Self::Triangle(a, c, d);
                    self.next()
                } else if adb.dot(a0) > 0.0 {
                    *self = Self::Triangle(a, d, b);
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
    pub fn push(&mut self, p: Vec3) {
        match self {
            Simplex::Point(a) => *self = Simplex::Line(p, *a),
            Simplex::Line(a, b) => *self = Simplex::Triangle(p, *a, *b),
            Simplex::Triangle(a, b, c) => *self = Simplex::Tetrahedron(p, *a, *b, *c),
            Simplex::Tetrahedron(_, _, _, _) => unreachable!(),
        }
    }
}

/// Performs a gjk intersection test.
/// Returns true if the shapes intersect.
pub fn intersection<A: CollisionPrimitive, B: CollisionPrimitive>(
    a_pos: Position,
    b_pos: Position,
    a_coll: &A,
    b_coll: &B,
) -> (bool, Simplex) {
    // Get first support function in direction of separation
    let dir = (a_pos - b_pos).normalized();
    let a = minkowski_diff(a_pos, b_pos, a_coll, b_coll, dir);

    let mut simplex = Simplex::Point(a);

    while let Some(dir) = simplex.next() {
        // Get the next simplex
        let p = minkowski_diff(a_pos, b_pos, a_coll, b_coll, dir);

        // New point was not past the origin
        // No collision
        if p.dot(dir) < 0.0 {
            return (false, simplex);
        }

        simplex.push(p);
    }

    // Collision found
    return (true, simplex);
}

// /// Expands the simplex into a better fit to enclose the origin.
// fn next_simplex(simplex: &mut Simplex) -> Option<Vec3> {
//     match simplex {
//         // Get the triangle that better encloses the origin
//         Simplex::Line(a, b) => {
//             dbg!("Line case");
//             // From the most recent point to the origin
//             let a0 = -*a;
//             let ab = *b - *a;
//             // Vec triple dot product
//             // dbg!(simplex);
//             let normal = triple_prod(ab, a0, ab);
//             // dbg!(normal);

//             Some(normal)
//         }
//         Simplex::Triangle(a, b, c) => {
//             dbg!("Triangle case");
//             // let a0 = -*a;
//             let ac = *c - *a;
//             let ab = *b - *a;
//             // let cb = *b - *c;

//             // Origin does not lie flat on the triangle. Remove oldest point
//             if let Some((normal, idx)) = simplex.to_origin() {
//                 simplex.remove(idx);

//                 eprintln!("Triangle does not contain origin");
//                 Some(normal)
//             }
//             // Simplex contains origin in the flat plane of the triangle. Expand
//             // in the next dimension
//             else {
//                 Some(ab.cross(ac))
//             }
//         }
//         Simplex::Tetrahedron(_, _, _, _) => {
//             dbg!("Tetrahedron case");
//             // Origin is outside simplex
//             if let Some((normal, idx)) = simplex.to_origin() {
//                 simplex.remove(idx);
//                 Some(normal)
//             }
//             // Origin is inside tetrahedron, collision
//             else {
//                 None
//             }
//         }
//     }
// }

/// Returns a point on the minkowski difference given from two colliders, their
/// transform, and a direction.
fn minkowski_diff<A: CollisionPrimitive, B: CollisionPrimitive>(
    a_pos: Position,
    b_pos: Position,
    a_coll: &A,
    b_coll: &B,
    dir: Vec3,
) -> Vec3 {
    let a = a_coll.support(dir) + *a_pos;
    let b = b_coll.support(-dir) + *b_pos;

    a - b
}
