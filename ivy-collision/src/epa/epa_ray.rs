use crate::{
    util::{plane_ray, ray_distance, SupportPoint, MAX_ITERATIONS, TOLERANCE},
    Contact, ContactPoints, Face, Polytype, Ray, Simplex,
};

use ultraviolet::Vec3;

pub fn epa_ray<F: Fn(Vec3) -> SupportPoint>(
    support_func: F,
    simplex: Simplex,
    ray: &Ray,
) -> Contact {
    let mut polytype =
        Polytype::from_simplex(&simplex, |a, b| Face::new_ray(a, b, ray, Vec3::zero()));

    let mut iterations = 0;
    loop {
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

        if iterations >= MAX_ITERATIONS
            || (support_distance.abs() - max_face.distance.abs()).abs() < TOLERANCE
        {
            return Contact {
                // points: ContactPoints::new(&[polytype[min_face.indices[0]].a]),
                points: ContactPoints::single(plane_ray(
                    polytype[max_face.indices[0]].a,
                    max_face.normal,
                    ray,
                )),
                depth: max_face.distance,
                normal: dir,
            };
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
