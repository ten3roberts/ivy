use super::PolytypeFace;
use glam::Vec3;

use crate::{
    util::{SupportPoint, TOLERANCE},
    PersistentContact, Contact, Polytype, Simplex,
};

pub fn epa(simplex: Simplex, support_func: impl Fn(Vec3) -> SupportPoint) -> Contact {
    let _span = tracing::debug_span!("epa").entered();
    let midpoint = simplex.points().iter().map(|v| v.p).sum::<Vec3>() / 4.0;

    assert_eq!(simplex.points().len(), 4);
    let mut polytype = Polytype::new(
        simplex.points(),
        &[
            0, 1, 2, //
            0, 3, 1, //
            0, 2, 3, //
            1, 3, 2, //
        ],
        PolytypeFace::new,
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
    loop {
        tracing::debug!(iterations);
        let (_, min_face) = if let Some(val) = polytype.find_closest_face() {
            val
        } else {
            panic!("Empty polytype");
        };
        // // assert_eq!(min.normal.mag(), 1.0);

        let new_support = support_func(min_face.normal);

        // let support_dist = min.normal.dot(new_support.support);

        // if (support_dist - min.distance) > TOLERANCE {
        assert!(min_face.normal.is_normalized());
        let d = new_support.p.dot(min_face.normal);

        tracing::debug!(?new_support, d, min_face.distance);
        if d - min_face.distance < TOLERANCE {
            let (point_a, point_b) = polytype.contact_points(min_face);
            return Contact {
                point_a,
                point_b,
                depth: min_face.distance,
                normal: min_face.normal,
            };
        }

        polytype.add_point(new_support, PolytypeFace::new);
        iterations += 1;
    }
}
