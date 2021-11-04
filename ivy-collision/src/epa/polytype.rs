use smallvec::{Array, SmallVec};
use std::{ops::Index, time::Duration};
use ultraviolet::Vec3;

use crate::{
    util::{barycentric_vector, ray_distance, SupportPoint},
    ContactPoints, Ray, Simplex,
};

#[derive(Clone, Copy, Debug)]
pub struct Face {
    pub indices: [u16; 3],
    pub normal: Vec3,
    pub distance: f32,
}

impl Face {
    /// Returns None if the normal cannot be calculated
    pub fn new(points: &[SupportPoint], indices: [u16; 3]) -> Self {
        let [p1, p2, p3] = [
            points[indices[0] as usize],
            points[indices[1] as usize],
            points[indices[2] as usize],
        ];

        let normal = (p2.support - p1.support)
            .cross(p3.support - p1.support)
            .normalized();

        // Distance to the origin of the minkowski difference
        let distance = normal.dot(p1.support);

        // Take care of handedness
        let normal = normal * distance.signum();
        let distance = distance.abs();

        Face {
            indices: [indices[0], indices[1], indices[2]],
            normal,
            distance,
        }
    }

    /// Creates a face where the normal is based on the edges and the reference to
    /// point away from it. Usually the reference point is the opposite vertex of
    /// the extended triangle.
    pub fn new_ray(
        points: &[SupportPoint],
        indices: [u16; 3],
        ray: &Ray,
        reference_point: Vec3,
    ) -> Self {
        let [p1, p2, p3] = [
            points[indices[0] as usize],
            points[indices[1] as usize],
            points[indices[2] as usize],
        ];

        let total_radial: Vec3 = [p1, p2].iter().map(|p| p.support - reference_point).sum();

        let normal = (p2.support - p1.support)
            .cross(p3.support - p1.support)
            .normalized();

        dbg!(normal.dot(total_radial));

        let normal = normal * normal.dot(total_radial).signum();

        //         // Point against ray
        //         let normal = normal * -normal.dot(ray.dir()).signum();

        //         // Calculate if face is front facing or back facing
        //         // Radial vector from the ray radially to the face midpoint
        //         let front = [p1, p2, p3]
        //             .iter()
        //             .map(|p| {
        //                 let radial = project_plane(p.pos, normal);
        //                 let dot = radial.dot(ray.dir());
        //                 eprintln!("Dot product: {:?}", dot);
        //                 dot
        //             })
        //             .all(|val| val < -TOLERANCE);

        // let normal = if check_triangle_intersect(&[p1.pos, p2.pos, p3.pos], ray.dir())
        // || radial.dot(normal) > 0.0
        // let normal = if front {
        //     // front
        //     eprintln!("Front face");
        //     normal
        // } else {
        //     // back
        //     eprintln!("Back face");
        //     normal * -1.0
        // };

        if normal.x.is_nan() || normal.y.is_nan() || normal.z.is_nan() {
            dbg!(
                indices,
                normal,
                p2.support - p1.support,
                p3.support - p1.support
            );
            std::thread::sleep(Duration::from_secs(100));
            panic!("");
        }

        let distance = ray_distance(p1, normal, ray);

        Face {
            indices: [indices[0], indices[1], indices[2]],
            normal,
            distance,
        }
    }

    pub fn a_points(&self, points: &[SupportPoint]) -> [Vec3; 3] {
        [
            points[self.indices[0] as usize].a,
            points[self.indices[1] as usize].a,
            points[self.indices[2] as usize].a,
        ]
    }

    pub fn middle(&self, points: &[SupportPoint]) -> Vec3 {
        (points[self.indices[0] as usize].support
            + points[self.indices[1] as usize].support
            + points[self.indices[2] as usize].support)
            / 3.0
    }

    pub fn edges(&self) -> [Edge; 3] {
        [
            (self.indices[0], self.indices[1]),
            (self.indices[0], self.indices[2]),
            (self.indices[1], self.indices[2]),
        ]
    }

    pub(crate) fn support_points(&self, points: &SmallVec<[SupportPoint; 32]>) -> [Vec3; 3] {
        [
            points[self.indices[0] as usize].support,
            points[self.indices[1] as usize].support,
            points[self.indices[2] as usize].support,
        ]
    }
}

type Edge = (u16, u16);

#[derive(Debug)]
pub struct Polytype {
    pub points: SmallVec<[SupportPoint; 32]>,
    // Normals and distances combined
    pub faces: SmallVec<[Face; 32]>,
}

impl Polytype {
    pub fn new<F: Fn(&[SupportPoint], [u16; 3]) -> Face>(
        points: &[SupportPoint],
        faces: &[u16],
        face_func: F,
    ) -> Self {
        let faces = faces
            .chunks_exact(3)
            .map(|val| face_func(points, [val[0], val[1], val[2]]))
            .collect();

        Self {
            points: points.into(),
            faces,
        }
    }

    pub fn find_furthest_face(&self) -> Option<(u16, Face)> {
        self.faces
            .iter()
            .inspect(|val| eprintln!("comparing: {:?}", val.distance))
            .enumerate()
            .filter(|(_, val)| val.distance.is_finite())
            .max_by(|a, b| a.1.distance.partial_cmp(&b.1.distance).unwrap())
            .map(|(a, b)| (a as u16, *b))
    }

    pub fn find_closest_face(&self) -> Option<(u16, Face)> {
        self.faces
            .iter()
            .inspect(|val| eprintln!("comparing: {:?}", val.distance))
            .enumerate()
            .filter(|(_, val)| val.distance.is_finite())
            .min_by(|a, b| a.1.distance.partial_cmp(&b.1.distance).unwrap())
            .map(|(a, b)| (a as u16, *b))
    }

    // Adds a point to the polytype
    // If the polytype collapses and add failed, false is returned
    pub fn add<F: Fn(&[SupportPoint], [u16; 3]) -> Face>(&mut self, p: SupportPoint, face_func: F) {
        // Special case when polytype is less than a tetrahedron
        // if self.points.len() == 3 {
        //     return self.expand(p, face_func);
        // }

        eprintln!(
            "Polytope: faces: {}, points: {}",
            self.faces.len(),
            self.points.len()
        );

        // remove faces that can see the point
        let mut edges = SmallVec::<[Edge; 16]>::new();
        let mut i = 0;

        while i < self.faces.len() {
            let face = self.faces[i];
            // Vector from a point on the face to the new point
            let to = p.support - self[face.indices[0]].support;

            // Dot product between current face normal and direction from face
            // to new point
            let dot = face.normal.dot(to);
            eprintln!("rel: {:?}, normal: {:?}", to, self.faces[i].normal);

            // Current face points into the new point
            if dot > 0.0 {
                // Only remove the face if the polytype is three dimensional
                let face = self.faces.swap_remove(i);
                for edge in face.edges() {
                    remove_or_add_edge(&mut edges, edge);
                }
            } else {
                i += 1;
            }
        }
        // add vertex
        let n = self.points.len();
        self.points.push(p);
        let points = &self.points;

        eprintln!("Edges: {}", edges.len());

        // add new faces
        let new_faces = edges
            .into_iter()
            .map(|(a, b)| face_func(points, [n as _, a, b]));

        eprintln!("New faces: {}, old: {}", new_faces.len(), self.faces.len());

        self.faces.extend(new_faces);
        // assert_ne!(self.faces.len(), 0);
    }

    // Adds a point to the polytype onto the specified face.
    // Polytype will not have any back faces
    pub fn add_decimate(&mut self, face: Face, p: SupportPoint, ray: &Ray) {
        // add vertex
        let n = self.points.len() as u16;
        self.points.push(p);
        let points = &self.points;

        let new_faces = [(0, 1, 2), (0, 2, 1), (1, 2, 0)].iter().map(|val| {
            Face::new_ray(
                points,
                [face.indices[val.0], face.indices[val.1], n],
                ray,
                points[face.indices[val.2] as usize].support,
            )
        });

        self.faces.clear();
        self.faces.extend(new_faces);
        assert_eq!(self.faces.len(), 3);
    }

    pub fn contact_points(&self, face: Face) -> ContactPoints {
        let [p1, p2, p3] = [
            self[face.indices[0]],
            self[face.indices[1]],
            self[face.indices[2]],
        ];

        let (u, v, w) = barycentric_vector(
            face.normal * face.distance,
            p1.support,
            p2.support,
            p3.support,
        );

        let a = p1.a * u + p2.a * v + p3.a * w;

        let b = p1.b * u + p2.b * v + p3.b * w;

        ContactPoints::double(a, b)
    }

    /// Constructs a polytype from a simplex.
    /// Currently only implemented for Triangle and Tetrahedron simplex
    pub fn from_simplex<F: Fn(&[SupportPoint], [u16; 3]) -> Face>(
        simplex: &Simplex,
        face_func: F,
    ) -> Self {
        match simplex {
            Simplex::Triangle(val) => Polytype::new(val, &[0, 1, 2], face_func),
            Simplex::Tetrahedron(val) => {
                Polytype::new(val, &[0, 1, 2, 0, 3, 1, 0, 2, 3, 1, 3, 2], face_func)
            }
            _ => todo!(),
        }
    }
}

impl Index<u16> for Polytype {
    type Output = SupportPoint;

    fn index(&self, index: u16) -> &Self::Output {
        &self.points[index as usize]
    }
}

fn remove_or_add_edge<T: Array<Item = Edge>>(edges: &mut SmallVec<T>, edge: Edge) {
    if let Some((index, _)) = edges.iter().enumerate().find(|(_, val)| {
        assert_ne!(**val, edge);
        (val.0, val.1) == (edge.1, edge.0)
    }) {
        edges.remove(index);
    } else {
        edges.push(edge);
    }
}
