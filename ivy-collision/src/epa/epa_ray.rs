use crate::{
    epa::ray_distance,
    util::{SupportPoint, MAX_ITERATIONS, TOLERANCE},
    Contact, ContactPoints, Face, Polytype, Ray, Simplex,
};

use ivy_core::{Color, Gizmo, Gizmos};
use ultraviolet::Vec3;

pub fn epa_ray<F: Fn(Vec3) -> SupportPoint>(
    support_func: F,
    simplex: Simplex,
    ray: &Ray,
    gizmos: &mut Gizmos,
) -> Contact {
    dbg!(&simplex);

    let mut polytype =
        Polytype::from_simplex(&simplex, |a, b| Face::new_ray(a, b, ray, Vec3::zero()));

    let mut iterations = 0;
    loop {
        dbg!(iterations);
        // Find the face closest to the ray
        let (index, min_face) = match polytype.find_closest_face() {
            Some(val) => val,
            None => {
                panic!("No intersecting faces");
            }
        };

        // Search in the normal of the face pointing against the ray

        let dir = min_face.normal * -min_face.normal.dot(ray.dir()).signum();

        // let mid = project_plane(
        //     (polytype[face.indices[0]].pos
        //         + polytype[face.indices[1]].pos
        //         + polytype[face.indices[2]].pos)
        //         / 3.0,
        //     ray.dir(),
        // );

        // let (closest_edge, edge_dist) = face.closest_edge(&polytype.points, ray);

        // let search_dir = (dir + mid.normalized()).normalized();
        let search_dir = dir;

        let p = support_func(search_dir);
        let distance = ray_distance(p.a, dir, ray);
        eprintln!("Searching in {:?}", dir);

        // let distance = ray_distance(p.a, dir, ray);
        eprintln!("{}, found {}", min_face.distance, distance);

        if iterations >= MAX_ITERATIONS || (distance - min_face.distance).abs() < TOLERANCE {
            // let face_points = ContactPoints::from_iter(polytype.points.iter().map(|val| val.a));
            let face_points = ContactPoints::new(&[
                polytype[min_face.indices[0]].pos,
                polytype[min_face.indices[1]].pos,
                polytype[min_face.indices[2]].pos,
            ]);

            gizmos.push(Gizmo::Sphere {
                origin: -min_face.distance * ray.dir(),
                color: Color::blue(),
                radius: 0.05,
                corner_radius: 1.0,
            });

            for (i, face) in polytype.faces.iter().enumerate() {
                // let color = Color::hsl(i as f32 * 30.0, distance / face.distance, 0.5);
                let (color, radius) = if face.normal.dot(ray.dir()) > 0.0 {
                    (Color::gray(), 0.005)
                } else {
                    (Color::yellow(), 0.01)
                };

                let mid = face.middle(&polytype.points);
                gizmos.push(ivy_core::Gizmo::Line {
                    origin: mid,
                    color,
                    dir: face.normal * 0.5,
                    radius,
                    corner_radius: 1.0,
                });

                let p = -ray_distance(polytype[face.indices[0]].a, face.normal, ray) * ray.dir();

                //                 gizmos.push(Gizmo::Sphere {
                //                     origin: p,
                //                     color,
                //                     radius: 0.01,
                //                     corner_radius: 1.0,
                //                 });

                for edge in face.edges() {
                    let [p1, p2] = [&polytype[edge.0], &polytype[edge.1]];
                    gizmos.push(ivy_core::Gizmo::Line {
                        origin: p1.pos,
                        color,
                        dir: (p2.pos - p1.pos),
                        radius,
                        corner_radius: 1.0,
                    })
                    // gizmos.push(ivy_core::Gizmo::Sphere {
                    //     origin: p.pos,
                    //     color: Color::white(),
                    //     radius: 0.1,
                    //     corner_radius: 1.0,
                    // })
                }
            }

            return Contact {
                points: ContactPoints::new(&[
                    p.pos,
                    face_points[0],
                    face_points[1],
                    face_points[2],
                ]),
                // points: ContactPoints::from_iter(polytype.points.iter().map(|val| val.pos)),
                // points: ContactPoints::new(&[
                //     polytype[face.indices[0]].a,
                //     polytype[face.indices[1]].a,
                //     polytype[face.indices[2]].a,
                // ]),
                depth: min_face.distance,
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
            // eprintln!("old: {:?}, new: {:?}", face.normal, -dir);
            let face = &mut polytype.faces[index as usize];
            // Invert the face normal to point back

            // If the polytype is a triangle, force the only face to be removed
            // and expanded from
            // if polytype.points.len() == 3 {
            // face.normal = dir;
            // }

            // Add the new point
            // polytype.add_decimate(p, dir, |a, b| Face::new_ray(a, b, ray));
            polytype.add_decimate(min_face, p, ray);
        }

        iterations += 1;
    }
}
