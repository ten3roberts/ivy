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
        &[0, 1, 2, 0, 3, 1, 0, 2, 3, 1, 3, 2],
        Face::new,
    );

    let mut iterations = 0;
    loop {
        let (_, min) = match polytype.find_closest_face() {
            Some(val) => val,
            None => {
                // eprintln!("The two shapes are the same");
                let p = support_func(Vec3::X);
                return Contact {
                    points: ContactPoints::double(p.a, p.b),
                    depth: p.support.length(),
                    normal: p.support.normalize(),
                };
                // let p = support(a_transform, a_transform_inv, a_coll, Vec3::unit_x());
                // return Intersection {
                //     points: [
                //         a_transform.extract_translation(),
                //         b_transform.extract_translation(),
                //     ],
                //     depth: p.mag(),
                //     normal: p.normalized(),
                // };
            }
        };

        // assert_eq!(min.normal.mag(), 1.0);

        let p = support_func(min.normal);

        let support_dist = min.normal.dot(p.support);

        if iterations < MAX_ITERATIONS && (support_dist - min.distance).abs() > TOLERANCE {
            polytype.add(p, Face::new);
        }
        // Support is further than the current closest normal
        else {
            return Contact {
                points: polytype.contact_points(min),
                depth: min.distance,
                normal: min.normal,
            };
        }

        iterations += 1;
    }
}
