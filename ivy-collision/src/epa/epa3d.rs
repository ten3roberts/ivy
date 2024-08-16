use super::Face;
use glam::Vec3;

use crate::{
    util::MAX_ITERATIONS,
    Contact, ContactPoints, Polytype, Simplex,
    {util::SupportPoint, util::TOLERANCE},
};

pub fn epa<F: Fn(Vec3) -> SupportPoint>(support_func: F, simplex: Simplex) -> Contact {
    assert_eq!(simplex.points().len(), 4);
    let mut polytype = Polytype::new(
        simplex.points(),
        &[
            0, 1, 2, //
            0, 3, 1, //
            0, 2, 3, //
            1, 3, 2, //
        ],
        Face::new,
    );

    let mut iterations = 0;
    loop {
        let (_, min) = if let Some(val) = polytype.find_closest_face() {
            val
        } else {
            panic!("Empty polytype");
            // // eprintln!("The two shapes are the same");
            // let p = support_func(Vec3::X);
            // return Contact {
            //     points: ContactPoints::double(p.a, p.b),
            //     depth: p.support.length(),
            //     normal: p.support.normalize(),
            // };
            // let p = support(a_transform, a_transform_inv, a_coll, Vec3::unit_x());
            // return Intersection {
            //     points: [
            //         a_transform.extract_translation(),
            //         b_transform.extract_translation(),
            //     ],
            //     depth: p.mag(),
            //     normal: p.normalized(),
            // };
        };

        if iterations > MAX_ITERATIONS {
            tracing::error!("reached max iterations");
            return Contact {
                points: polytype.contact_points(min),
                depth: min.distance,
                normal: min.normal,
                polytype,
            };
        }
        // assert_eq!(min.normal.mag(), 1.0);

        let new_support = support_func(min.normal);

        let support_dist = min.normal.dot(new_support.support);

        if (support_dist - min.distance) > TOLERANCE {
            polytype.add_point(new_support, Face::new);
        } else {
            return Contact {
                points: polytype.contact_points(min),
                depth: min.distance,
                normal: min.normal,
                polytype,
            };
        }

        iterations += 1;
    }
}
