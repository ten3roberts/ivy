use crate::{
    epa::ray_distance,
    util::{SupportPoint, MAX_ITERATIONS, TOLERANCE},
    Contact, ContactPoints, Face, Polytype, Ray, Simplex,
};

use ultraviolet::Vec3;

pub fn epa_ray<F: Fn(Vec3) -> SupportPoint>(
    support_func: F,
    simplex: Simplex,
    ray: &Ray,
) -> Contact {
    dbg!(&simplex);
    let mut polytype = Polytype::from_simplex(&simplex, |a, b| Face::new_ray(a, b, ray));

    let mut iterations = 0;
    loop {
        dbg!(iterations);
        // Find the face closest to the ray
        let (min_index, min) = match polytype.find_furthest_face() {
            Some(val) => val,
            None => {
                todo!();
            }
        };

        // Search in the normal of the face pointing against the ray
        let dir = min.normal * -min.normal.dot(ray.dir()).signum();

        eprintln!("Looking in {:?}, {}", dir, dir.dot(ray.dir()));
        let p = support_func(dir);
        let distance = ray_distance(p.a, min.normal, ray);
        eprintln!(
            "New distance: {}, old: {}, diff: {}",
            distance,
            min.distance,
            (distance - min.distance).abs()
        );

        if iterations >= MAX_ITERATIONS || (distance - min.distance).abs() < TOLERANCE {
            return Contact {
                // points: ContactPoints::from_iter(polytype.points.iter().map(|val| val.a)),
                points: ContactPoints::new(&[
                    polytype[min.indices[0]].a,
                    polytype[min.indices[1]].a,
                    polytype[min.indices[2]].a,
                ]),
                depth: min.distance,
                normal: dir,
            };

            // let p1 = polytype[min.indices[0]];

            //             let p = plane_ray(p1.a, min.normal, ray);

            //             return Contact {
            //                 // points: [p_min, p_max],
            //                 points: [p].into(),
            //                 depth: min.distance,
            //                 normal: min.normal,
            //             };
        }
        // Support is further than the current closest face
        else {
            let face = &mut polytype.faces[min_index as usize];
            // eprintln!("old: {:?}, new: {:?}", face.normal, -dir);
            // face.normal = -dir;
            // Invert the face normal to point back
            // face.normal = -dir;

            // Never consider this face again
            // if polytype.points.len() == 3 {
            face.normal = dir;
            // }

            // Add the new point
            polytype.add_decimate(p, |a, b| Face::new_ray(a, b, ray));
        }

        iterations += 1;
    }
}
