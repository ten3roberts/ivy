use smallvec::SmallVec;
use ultraviolet::{Mat4, Vec3};

use crate::{
    gjk::Simplex,
    util::{project_plane, support},
    Intersection, Ray,
    {util::SupportPoint, util::TOLERANCE, CollisionPrimitive},
};

#[derive(Debug, Clone, Copy, PartialEq)]
struct SignedNormal {
    normal: Vec3,
    distance: f32,
}

#[derive(Debug)]
struct Polytype {
    points: SmallVec<[SupportPoint; 32]>,
}

impl Polytype {
    pub fn new(points: &[SupportPoint]) -> Self {
        Self {
            points: points.into(),
        }
    }

    pub fn get_normal(a: Vec3, b: Vec3, up: Vec3) -> SignedNormal {
        let ab = b - a;
        let n = up.cross(ab).normalized();
        let distance = n.dot(project_plane(a, up));

        SignedNormal {
            normal: n * distance.signum(),
            distance: distance.abs(),
        }
    }

    fn find_closest_point(&self, up: Vec3) -> Option<(u16, SignedNormal)> {
        self.points
            .windows(2)
            .enumerate()
            .map(|(i, val)| (i, Self::get_normal(val[0].pos, val[1].pos, up)))
            .min_by(|a, b| {
                a.1.distance
                    .partial_cmp(&b.1.distance)
                    .expect("Failed to compare float")
            })
            .map(|(a, b)| (a as u16, b))
    }

    // Adds a point to the polytype
    // If the polytype collapses and add failed, false is returned
    pub fn splice(&mut self, index: u16, p: SupportPoint) {
        self.points.insert(index as usize + 1, p)
    }

    // fn contact_points(&self, face: Face) -> [Vec3; 2] {
    //     let (u, v, w) = barycentric_vector(
    //         face.normal * face.distance,
    //         self.points[face.indices[0] as usize].pos,
    //         self.points[face.indices[1] as usize].pos,
    //         self.points[face.indices[2] as usize].pos,
    //     );

    //     let a = self.points[face.indices[0] as usize].a * u
    //         + self.points[face.indices[1] as usize].a * v
    //         + self.points[face.indices[2] as usize].a * w;

    //     let b = self.points[face.indices[0] as usize].b * u
    //         + self.points[face.indices[1] as usize].b * v
    //         + self.points[face.indices[2] as usize].b * w;

    //     [a, b]
    // }
}

pub fn epa_ray<T: CollisionPrimitive>(
    transform: &Mat4,
    transform_inv: &Mat4,
    collider: &T,
    simplex: Simplex,
    ray: &Ray,
) -> Intersection {
    assert_eq!(simplex.points().len(), 3);
    let mut polytype = Polytype::new(simplex.points());

    let mut iterations = 0;
    loop {
        let (i, min) = match polytype.find_closest_point(ray.dir()) {
            Some(val) => val,
            None => {
                eprintln!("The two shapes are the same");
                let p = support(transform, transform_inv, collider, Vec3::unit_x());
                return Intersection {
                    points: [
                        transform.extract_translation(),
                        transform.extract_translation(),
                    ],
                    depth: p.mag(),
                    normal: p.normalized(),
                };
            }
        };

        dbg!(iterations);
        let p = ray.support(collider, transform, transform_inv, min.normal);

        let support_dist = min.normal.dot(p.pos);

        if iterations < 10 && (support_dist - min.distance).abs() > TOLERANCE {
            polytype.splice(i, p);
        }
        // Support is further than the current closest normal
        else {
            return Intersection {
                points: Default::default(),
                depth: min.distance,
                normal: min.normal,
            };
        }

        iterations += 1;
    }
}
