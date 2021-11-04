use crate::{
    util::{plane_ray, ray_distance, SupportPoint, MAX_ITERATIONS, TOLERANCE},
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
        let (_index, max_face) = match polytype.find_furthest_face() {
            Some(val) => val,
            None => {
                unreachable!("No intersecting faces");
            }
        };

        // Search in the normal of the face pointing against the ray

        let dir = max_face.normal * -max_face.normal.dot(ray.dir()).signum();

        // let mid = project_plane(
        //     (polytype[face.indices[0]].pos
        //         + polytype[face.indices[1]].pos
        //         + polytype[face.indices[2]].pos)
        //         / 3.0,
        //     ray.dir(),
        // );

        // let (closest_edge, edge_dist) = face.closest_edge(&polytype.points, ray);

        // let search_dir = (dir + mid.normalized()).normalized();

        let p = support_func(dir);
        let support_distance = ray_distance(p, max_face.normal, ray);
        eprintln!("Searching in {:?}", dir);

        // let distance = ray_distance(p.a, dir, ray);
        eprintln!("{}, found {}", max_face.distance, support_distance);

        if iterations >= MAX_ITERATIONS
            || (support_distance.abs() - max_face.distance.abs()).abs() < TOLERANCE
        {
            gizmos.push(Gizmo::Sphere {
                origin: max_face.distance * ray.dir(),
                color: Color::blue(),
                radius: 0.05,
            });

            gizmos.push(Gizmo::Triangle {
                color: Color::red(),
                points: max_face.a_points(&polytype.points),
                radius: 0.01,
            });

            gizmos.push(Gizmo::Triangle {
                color: Color::red(),
                points: max_face.support_points(&polytype.points),
                radius: 0.01,
            });

            eprintln!("Max dot: {:?}", max_face.normal.dot(ray.dir()).signum());

            for face in polytype.faces.iter() {
                // let color = Color::hsl(i as f32 * 30.0, distance / face.distance, 0.5);
                let radius = 0.005;
                let color = if face.normal.dot(ray.dir()) > 0.0 {
                    Color::gray()
                } else {
                    Color::yellow()
                };

                let mid = face.middle(&polytype.points);
                gizmos.push(ivy_core::Gizmo::Line {
                    origin: mid,
                    color,
                    dir: face.normal * 0.5,
                    radius,
                    corner_radius: 1.0,
                });

                let p = ray_distance(polytype[face.indices[0]], face.normal, ray) * ray.dir();

                gizmos.push(Gizmo::Sphere {
                    origin: p,
                    color,
                    radius: 0.01,
                });

                gizmos.push(Gizmo::Triangle {
                    color,
                    points: face.support_points(&polytype.points),
                    radius: 0.01,
                });
            }

            return Contact {
                // points: ContactPoints::new(&[polytype[min_face.indices[0]].a]),
                points: ContactPoints::single(plane_ray(
                    polytype[max_face.indices[0]].a,
                    max_face.normal,
                    ray,
                )),
                // points: ContactPoints::new(&[
                //     p.pos,
                //     face_points[0],
                //     face_points[1],
                //     face_points[2],
                // ]),
                // // points: ContactPoints::from_iter(polytype.points.iter().map(|val| val.pos)),
                // // points: ContactPoints::new(&[
                // //     polytype[face.indices[0]].a,
                // //     polytype[face.indices[1]].a,
                // //     polytype[face.indices[2]].a,
                // // ]),
                depth: max_face.distance,
                normal: dir,
            };
            // },
            //     color: todo!(),
            //     points: todo!(),
            //     radius: todo!(), ;

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
            // Add the new point
            // polytype.add_decimate(p, dir, |a, b| Face::new_ray(a, b, ray));
            polytype.add_decimate(max_face, p, ray);
        }

        iterations += 1;
    }
}
