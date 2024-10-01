// Adapted from <https://github.com/vaiorabbit/MPRTest>
//
// Copyright (c) 2008-2022 vaiorabbit <http://twitter.com/vaiorabbit>
//
// This software is provided 'as-is', without any express or implied
// warranty. In no event will the authors be held liable for any damages
// arising from the use of this software.
//
// Permission is granted to anyone to use this software for any purpose,
// including commercial applications, and to alter it and redistribute it
// freely, subject to the following restrictions:
//
//     1. The origin of this software must not be misrepresented; you must not
//     claim that you wrote the original software. If you use this software
//     in a product, an acknowledgment in the product documentation would be
//     appreciated but is not required.
//
//     2. Altered source versions must be plainly marked as such, and must not be
//     misrepresented as being the original software.
//
//     3. This notice may not be removed or altered from any source
//     distribution.
use std::mem;

use glam::Vec3;

use crate::{util::SupportPoint, Shape};

pub struct Mpr {}

const TOL: f32 = 1e-6;

impl Mpr {
    pub fn new() -> Self {
        Self {}
    }

    pub fn intersect(&self, a: &impl Shape, b: &impl Shape) -> bool {
        let support = |dir: Vec3| {
            let a = a.support(-dir);
            let b = b.support(dir);

            SupportPoint { p: b - a, a, b }
        };
        // the first point of the projected tet is the geometric center.
        // This is guaranteed to lie within the minkowski difference of the intersecting shapes.
        let va0 = a.center();
        let vb0 = b.center();
        let mut v0 = SupportPoint {
            p: vb0 - va0,
            a: va0,
            b: vb0,
        };

        if v0.length() < TOL {
            v0.p = Vec3::X * 0.001;
        }

        let dir1 = -v0.normalize();
        let mut v1 = support(dir1);

        // v1 was not past origin
        if v1.p.dot(dir1) <= 0.0 {
            return false;
        }

        let dir2 = v1.p.cross(v0.p);

        if dir2.length() < TOL {
            return true;
        }

        let mut v2 = support(dir2.normalize());

        // v2 did not pass origin
        if v2.p.dot(dir2) <= 0.0 {
            return false;
        }

        let mut v3: SupportPoint = Default::default();
        loop {
            let normal = (v1.p - v0.p).cross(v2.p - v0.p).normalize();

            let v3 = support(normal);

            if (v3.p.dot(normal)) <= 0.0 {
                return false;
            }

            if normal.dot(dir1) < 0.0 {
                mem::swap(&mut v1, &mut v2);
                continue;
            }

            if (v3.p.cross(v2.p).dot(v0.p)) < 0.0 {
                v1 = v3;
                continue;
            }

            if v1.p.cross(v3.p).dot(v0.p) < 0.0 {
                v2 = v3;
                continue;
            }

            break;
        }

        loop {
            let portal = (v2.p - v1.p).cross(v3.p - v1.p).normalize();

            // This face is *past the origin
            // v0 and (v1,v2,v3) now form a tetrahedron enclosing the origin
            if portal.dot(v1.p) >= 0.0 {
                return true;
            }

            let v4 = support(portal);

            if v4.p.dot(portal) <= TOL || (v4.p - v3.p).dot(portal) <= TOL {
                return false;
            }

            if v4.p.cross(v1.p).dot(v0.p) < 0.0 {
                if v4.p.cross(v2.p).dot(v0.p) < 0.0 {
                    v1 = v4; // (v2, v3, v4), discard v1
                } else {
                    v3 = v4; // (v1, v2, v4), discard v4
                }
            } else if (v4.p.cross(v3.p).dot(v0.p)) < 0.0 {
                v2 = v4; // (v1, v3, v4), discard v2
            } else {
                v1 = v4; // (v2, v3, v4), discard v1
            }
        }
    }
}

impl Default for Mpr {
    fn default() -> Self {
        Self::new()
    }
}

pub struct MprResult {}

#[derive(Debug, Clone)]
pub struct MprContact {
    pub v0: SupportPoint,
    pub v1: SupportPoint,
    pub v2: SupportPoint,
    pub v3: SupportPoint,
}

#[cfg(test)]
mod test {
    use glam::{Mat4, Quat, Vec3};

    use crate::{BoundingBox, TransformedShape};

    use super::Mpr;

    #[test]
    fn text_box_box() {
        let a = BoundingBox::new(Vec3::ONE, Vec3::ZERO);
        let b = BoundingBox::new(Vec3::ONE, Vec3::ZERO);

        let mpr = Mpr::new();

        for i in 0..=20 {
            let a_pos = -Vec3::X * 1.1 + Vec3::X * (i as f32 * 0.1);
            let a_rot = Quat::from_rotation_x((i % 10) as f32 * 0.1);
            let b_pos = Vec3::X;

            let intersection = mpr.intersect(
                &TransformedShape::new(a, Mat4::from_rotation_translation(a_rot, a_pos)),
                &TransformedShape::new(b, Mat4::from_translation(b_pos)),
            );
            // let intersection2 = crate::gjk(
            //     &TransformedShape::new(a, Mat4::from_translation(a_pos)),
            //     &TransformedShape::new(b, Mat4::from_translation(b_pos)),
            // );

            eprintln!("{i} {intersection:?}");
            // assert!(result.0);
        }
    }
}
