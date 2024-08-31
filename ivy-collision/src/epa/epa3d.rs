use super::Face;
use glam::Vec3;

use crate::{
    util::MAX_ITERATIONS,
    ContactPoints, Intersection, Polytype, Simplex,
    {util::SupportPoint, util::TOLERANCE},
};

pub fn epa(simplex: Simplex, support_func: impl Fn(Vec3) -> SupportPoint) -> Intersection {
    let _span = tracing::info_span!("epa").entered();
    let midpoint = simplex.points().iter().map(|v| v.support).sum::<Vec3>() / 4.0;

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

    // for face in &polytype.faces {
    //     let p1 = polytype.points[face.indices[0] as usize].support;
    //     let p2 = polytype.points[face.indices[1] as usize].support;
    //     let p3 = polytype.points[face.indices[2] as usize].support;

    //     let face_midpoint = (p1 + p2 + p3) / 3.0;

    //     assert!(face.normal.dot(face_midpoint - midpoint) > 0.0);
    // }

    // return Contact {
    //     points: polytype.contact_points(polytype.faces[0]),
    //     depth: 0.0,
    //     normal: Default::default(),
    //     polytype,
    // };

    let mut iterations = 0;
    let iteration_count = 64;
    loop {
        tracing::info!(iterations);
        let (_, min) = if let Some(val) = polytype.find_closest_face() {
            val
        } else {
            panic!("Empty polytype");
        };
        // // assert_eq!(min.normal.mag(), 1.0);

        let new_support = support_func(min.normal);

        // let support_dist = min.normal.dot(new_support.support);

        // if (support_dist - min.distance) > TOLERANCE {
        assert!(min.normal.is_normalized());
        let d = new_support.support.dot(min.normal);

        tracing::info!(?new_support, d, min.distance);
        if (d - min.distance < TOLERANCE) {
            return Intersection {
                points: polytype.contact_points(min),
                depth: min.distance,
                normal: min.normal,
                polytype,
            };
        }

        // if iterations >= iteration_count {
        //     tracing::error!("reached max iterations");
        //     return Intersection {
        //         points: polytype.contact_points(min),
        //         depth: min.distance,
        //         normal: min.normal,
        //         polytype,
        //     };
        // }

        polytype.add_point(new_support, Face::new);
        iterations += 1;
    }
}
