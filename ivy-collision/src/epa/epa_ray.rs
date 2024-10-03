use crate::{
    util::{plane_ray, ray_distance, SupportPoint, TOLERANCE},
    Contact, Polytype, PolytypeFace, Ray, Simplex,
};

use glam::Vec3;
use rayon::iter;

pub fn epa_ray<F: Fn(Vec3) -> SupportPoint>(
    support_func: F,
    simplex: Simplex,
    ray: &Ray,
) -> Contact {
    let mut polytype = Polytype::from_simplex(&simplex, |a, b| {
        PolytypeFace::new_ray(a, b, ray, Vec3::ZERO)
    });

    let mut iterations = 0;
    loop {
        iterations += 1;
        // Find the face closest to the ray
        let (_index, max_face) = match polytype.find_furthest_face() {
            Some(val) => val,
            None => {
                unreachable!("No intersecting faces");
            }
        };

        // Search in the normal of the face pointing against the ray
        let dir = max_face.normal * -max_face.normal.dot(ray.dir()).signum();

        let p = support_func(dir);
        let support_distance = ray_distance(p, max_face.normal, ray);
        if iterations > 1000 {
            tracing::error!("max epa iterations reached");
            let point = plane_ray(polytype[max_face.indices[0]].a, max_face.normal, ray);

            return Contact {
                // points: ContactPoints::new(&[polytype[min_face.indices[0]].a]),
                point_a: point,
                point_b: point,
                depth: (point - ray.origin).length(),
                normal: dir,
            };
        }
        if (support_distance.abs() - max_face.distance.abs()).abs() < TOLERANCE {
            let point = plane_ray(polytype[max_face.indices[0]].a, max_face.normal, ray);

            return Contact {
                // points: ContactPoints::new(&[polytype[min_face.indices[0]].a]),
                point_a: point,
                point_b: point,
                depth: (point - ray.origin).length(),
                normal: dir,
            };
        }
        // Support is further than the current closest face
        else {
            // Add the new point
            // polytype.add_decimate(p, dir, |a, b| Face::new_ray(a, b, ray));
            polytype.add_decimate(max_face, p, ray);
        }
    }
}
