// use std::ops::Index;

// use smallvec::SmallVec;
// use glam::{Mat4, Vec3};

// use crate::{
//     gjk::Simplex,
//     util::{normal, support},
//     Contact, Ray,
//     {util::SupportPoint, util::TOLERANCE, CollisionPrimitive},
// };

// #[derive(Debug, Clone, Copy, PartialEq)]
// struct Edge {
//     indices: [u16; 2],
//     normal: Vec3,
//     distance: f32,
// }

// impl Edge {
//     pub fn new(points: &[SupportPoint], indices: &[u16], up: Vec3) -> Self {
//         let a = points[indices[0] as usize];
//         let b = points[indices[1] as usize];

//         let ab = b.pos - a.pos;
//         // let n = up.cross(ab).normalized();
//         let n = (a.pos + b.pos).normalized();
//         let distance = n.dot(a.pos);

//         Self {
//             indices: [indices[0], indices[1]],
//             normal: n * distance.signum(),
//             distance: distance.abs(),
//         }
//     }
// }

// #[derive(Debug)]
// struct Polytype {
//     points: SmallVec<[SupportPoint; 32]>,
// }

// impl Polytype {
//     pub fn new(points: &[SupportPoint]) -> Self {
//         Self {
//             points: points.into(),
//         }
//     }

//     fn find_closest_point(&self, up: Vec3) -> Option<Edge> {
//         self.points
//             .windows(2)
//             .enumerate()
//             .map(|(i, _)| Edge::new(&self.points, &[i as u16, i as u16 + 1], up))
//             .min_by(|a, b| {
//                 a.distance
//                     .partial_cmp(&b.distance)
//                     .expect("Failed to compare float")
//             })
//     }

//     fn find_furthest_point(&self, up: Vec3) -> Option<Edge> {
//         self.points
//             .windows(2)
//             .enumerate()
//             .map(|(i, _)| Edge::new(&self.points, &[i as u16, i as u16 + 1], up))
//             .max_by(|a, b| {
//                 a.distance
//                     .partial_cmp(&b.distance)
//                     .expect("Failed to compare float")
//             })
//     }
//     // Adds a point to the polytype
//     // If the polytype collapses and add failed, false is returned
//     pub fn splice(&mut self, edge: &Edge, p: SupportPoint) {
//         self.points.insert(edge.indices[1] as usize, p)
//     }

//     fn contact_points<T: CollisionPrimitive>(
//         &self,
//         transform: &Mat4,
//         transform_inv: &Mat4,
//         collider: &T,
//         edge: Edge,
//         ray: &Ray,
//         depth: f32,
//     ) -> Contact {
//         let a = self[edge.indices[0]];
//         let b = self[edge.indices[1]];

//         // let b = &simplex[edge.index];
//         // let a = if edge.index == 0 {
//         //     &simplex[simplex.len() - 1]
//         // } else {
//         //     &simplex[edge.index - 1]
//         // };
//         let oa = -a.pos;
//         let ab = b.pos - a.pos;
//         let t = oa.dot(ab) / ab.mag_sq();

//         let p1 = if t < 0.0 {
//             a.a
//         } else if t > 1.0 {
//             b.a
//         } else {
//             a.a + (b.a - a.a) * t
//         };

//         let p2 = if t < 0.0 {
//             b.a
//         } else if t > 1.0 {
//             a.a
//         } else {
//             b.a + (a.a - b.a) * t
//         };
//         let n1 = (normal(
//             transform,
//             transform_inv,
//             collider,
//             (p1 - transform.extract_translation()).normalized(),
//         ) + normal(transform, transform_inv, collider, -ray.dir()))
//         .normalized();

//         let n2 = normal(
//             transform,
//             transform_inv,
//             collider,
//             -(ray.origin() - transform.extract_translation()).normalized(),
//         );

//         // let p1 = plane_ray(p1, n1, ray);
//         // let p2 = plane_ray(p2, n2, ray);
//         // Plane intersection solving singularities
//         // let p1 = if ((p1 - ray.origin()).normalized().dot(ray.dir()) - 1.0).abs() < TOLERANCE {
//         //     p1
//         // } else {
//         //     plane_ray(p1, n1, ray)
//         // };

//         // let p2 = if ((p2 - ray.origin()).normalized().dot(ray.dir()) - 1.0).abs() < TOLERANCE {
//         //     p2
//         // } else {
//         //     plane_ray(p2, n2, ray)
//         // };

//         // Sort based on distance
//         let points = if (p1 - ray.origin()).dot(ray.dir()) < (p2 - ray.origin()).dot(ray.dir()) {
//             [p1, p2]
//         } else {
//             [p2, p1]
//         };

//         Contact {
//             points: [a.a, b.a].into(),
//             depth,
//             normal: n1,
//         }
//     }

//     // fn contact_points(&self, face: Face) -> [Vec3; 2] {
//     //     let (u, v, w) = barycentric_vector(
//     //         face.normal * face.distance,
//     //         self.points[face.indices[0] as usize].pos,
//     //         self.points[face.indices[1] as usize].pos,
//     //         self.points[face.indices[2] as usize].pos,
//     //     );

//     //     let a = self.points[face.indices[0] as usize].a * u
//     //         + self.points[face.indices[1] as usize].a * v
//     //         + self.points[face.indices[2] as usize].a * w;

//     //     let b = self.points[face.indices[0] as usize].b * u
//     //         + self.points[face.indices[1] as usize].b * v
//     //         + self.points[face.indices[2] as usize].b * w;

//     //     [a, b]
//     // }
// }

// impl Index<u16> for Polytype {
//     type Output = SupportPoint;

//     fn index(&self, index: u16) -> &Self::Output {
//         &self.points[index as usize]
//     }
// }

// pub fn epa_ray<T: CollisionPrimitive>(
//     transform: &Mat4,
//     transform_inv: &Mat4,
//     collider: &T,
//     simplex: Simplex,
//     ray: &Ray,
// ) -> Contact {
//     assert_eq!(simplex.points().len(), 3);
//     let mut polytype = Polytype::new(simplex.points());

//     let mut iterations = 0;
//     loop {
//         let min = match polytype.find_closest_point(ray.dir()) {
//             Some(val) => val,
//             None => {
//                 eprintln!("The two shapes are the same");
//                 let p = support(transform, transform_inv, collider, Vec3::unit_x());
//                 return Contact {
//                     points: [
//                         transform.extract_translation(),
//                         transform.extract_translation(),
//                     ]
//                     .into(),
//                     depth: p.mag(),
//                     normal: p.normalized(),
//                 };
//             }
//         };

//         let p = ray.support(collider, transform, transform_inv, min.normal);

//         let support_dist = min.normal.dot(p.pos);

//         if iterations < 10 && (support_dist - min.distance).abs() > TOLERANCE {
//             polytype.splice(&min, p);
//         }
//         // Support is further than the current closest normal
//         else {
//             // let p = polytype[i];
//             // let a = p.a + min.normal.cross(ray.dir());
//             // return Intersection {
//             //     points: [p.a, p.a],
//             //     depth: min.distance,
//             //     normal: min.normal,
//             // };
//             return polytype.contact_points(
//                 transform,
//                 transform_inv,
//                 collider,
//                 min,
//                 ray,
//                 min.distance,
//             );
//         }

//         iterations += 1;
//     }
// }
