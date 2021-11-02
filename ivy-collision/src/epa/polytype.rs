use smallvec::{Array, SmallVec};
use std::ops::Index;
use ultraviolet::Vec3;

use crate::{
    util::{barycentric_vector, SupportPoint},
    ContactPoints, Ray, Simplex,
};

use super::ray_distance;

#[derive(Clone, Copy, Debug)]
pub struct Face {
    pub indices: [u16; 3],
    pub normal: Vec3,
    pub distance: f32,
}

impl Face {
    /// Returns None if the normal cannot be calculated
    pub fn new(points: &[SupportPoint], indices: &[u16]) -> Self {
        let [p1, p2, p3] = [
            points[indices[0] as usize],
            points[indices[1] as usize],
            points[indices[2] as usize],
        ];

        let normal = (p2.pos - p1.pos).cross(p3.pos - p1.pos).normalized();

        // Distance to the origin of the minkowski difference
        let distance = normal.dot(p1.pos);

        // Take care of handedness
        let normal = normal * distance.signum();
        let distance = distance.abs();

        Face {
            indices: [indices[0], indices[1], indices[2]],
            normal,
            distance,
        }
    }

    /// Same as new except calculated the distance based on alignment to a
    /// idrection
    pub fn new_ray(points: &[SupportPoint], indices: &[u16], ray: &Ray) -> Self {
        let [p1, p2, p3] = [
            points[indices[0] as usize],
            points[indices[1] as usize],
            points[indices[2] as usize],
        ];

        let normal = (p2.pos - p1.pos).cross(p3.pos - p1.pos).normalized();

        // Point against ray
        // let normal = normal * -normal.dot(ray.dir()).signum();
        eprintln!("Created face with: {:?}, {}", normal, normal.dot(ray.dir()));

        if normal.x.is_nan() || normal.y.is_nan() || normal.z.is_nan() {
            dbg!(indices, normal, p2.pos - p1.pos, p3.pos - p1.pos);
            panic!("");
        }

        let distance = ray_distance(p1.a, normal, ray);

        Face {
            indices: [indices[0], indices[1], indices[2]],
            normal,
            distance,
        }
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
    pub fn new<F: Fn(&[SupportPoint], &[u16]) -> Face>(
        points: &[SupportPoint],
        faces: &[u16],
        face_func: F,
    ) -> Self {
        let faces = faces
            .chunks_exact(3)
            .map(|val| face_func(points, val))
            .collect();

        Self {
            points: points.into(),
            faces,
        }
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

    pub fn find_furthest_face(&self) -> Option<(u16, Face)> {
        self.faces
            .iter()
            .inspect(|val| eprintln!("comparing: {:?}", val.distance))
            .enumerate()
            .filter(|(_, val)| val.distance.is_finite())
            .max_by(|a, b| a.1.distance.partial_cmp(&b.1.distance).unwrap())
            .map(|(a, b)| (a as u16, *b))
    }

    // Expands the simplex without removing any faces but flips them instead
    pub fn expand<F: Fn(&[SupportPoint], &[u16]) -> Face>(
        &mut self,
        p: SupportPoint,
        face_func: F,
    ) {
        eprintln!("Expanding");
        // remove faces that can see the point
        let mut edges = SmallVec::<[Edge; 16]>::new();

        let faces = &mut self.faces;
        let points = &mut self.points;
        for face in faces.iter_mut() {
            // Vector from a point on the face to the new point
            let to = p.pos - points[face.indices[0] as usize].pos;

            // Dot product between current face normal and direction from face
            // to new point
            let dot = face.normal.dot(to);
            eprintln!("rel: {:?}, normal: {:?}", to, face.normal);

            // Current face points into the new point
            if dot > 0.0 {
                eprintln!("Exapnding into");
                // Only remove the face if the polytype is three dimensional
                remove_or_add_edge(&mut edges, (face.indices[0], face.indices[1]));
                remove_or_add_edge(&mut edges, (face.indices[1], face.indices[2]));
                remove_or_add_edge(&mut edges, (face.indices[2], face.indices[0]));

                // Flip face
                eprintln!("Flipping face");
                face.normal *= -1.0;
                // face.normal = face.normal * face.normal.dot(ray.dir()).signum();
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
            .map(|(a, b)| face_func(points, &[n as _, a, b]));

        eprintln!("New faces: {}, old: {}", new_faces.len(), self.faces.len());

        self.faces.extend(new_faces);
        // assert_ne!(self.faces.len(), 0);
    }

    // Adds a point to the polytype
    // If the polytype collapses and add failed, false is returned
    pub fn add<F: Fn(&[SupportPoint], &[u16]) -> Face>(&mut self, p: SupportPoint, face_func: F) {
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
            // Vector from a point on the face to the new point
            let to = p.pos - self[self.faces[i].indices[0]].pos;

            // Dot product between current face normal and direction from face
            // to new point
            let dot = self.faces[i].normal.dot(to);
            eprintln!("rel: {:?}, normal: {:?}", to, self.faces[i].normal);

            // Current face points into the new point
            if dot > 0.0 {
                // Only remove the face if the polytype is three dimensional
                let face = self.faces.swap_remove(i);
                remove_or_add_edge(&mut edges, (face.indices[0], face.indices[1]));
                remove_or_add_edge(&mut edges, (face.indices[1], face.indices[2]));
                remove_or_add_edge(&mut edges, (face.indices[2], face.indices[0]));
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
            .map(|(a, b)| face_func(points, &[n as _, a, b]));

        eprintln!("New faces: {}, old: {}", new_faces.len(), self.faces.len());

        self.faces.extend(new_faces);
        // assert_ne!(self.faces.len(), 0);
    }

    // Adds a point to the polytype
    // If the polytype collapses and add failed, false is returned.
    // Removes all other faces so that only the expanding front is preserved
    pub fn add_decimate<F: Fn(&[SupportPoint], &[u16]) -> Face>(
        &mut self,
        p: SupportPoint,
        face_func: F,
    ) {
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

        for face in &self.faces {
            // Vector from a point on the face to the new point
            let to = p.pos - self[face.indices[0]].pos;

            // Dot product between current face normal and direction from face
            // to new point
            let dot = face.normal.dot(to);
            eprintln!("rel: {:?}, normal: {:?}", to, face.normal);

            // Current face points into the new point
            if dot > 0.0 {
                // Only remove the face if the polytype is three dimensional
                remove_or_add_edge(&mut edges, (face.indices[0], face.indices[1]));
                remove_or_add_edge(&mut edges, (face.indices[1], face.indices[2]));
                remove_or_add_edge(&mut edges, (face.indices[2], face.indices[0]));
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
            .map(|(a, b)| face_func(points, &[n as _, a, b]));

        self.faces.clear();
        eprintln!("New faces: {}, old: {}", new_faces.len(), self.faces.len());

        self.faces.extend(new_faces);
        // assert_ne!(self.faces.len(), 0);
    }
    pub fn contact_points(&self, face: Face) -> ContactPoints {
        let [p1, p2, p3] = [
            self[face.indices[0]],
            self[face.indices[1]],
            self[face.indices[2]],
        ];

        let (u, v, w) = barycentric_vector(face.normal * face.distance, p1.pos, p2.pos, p3.pos);

        let a = p1.a * u + p2.a * v + p3.a * w;

        let b = p1.b * u + p2.b * v + p3.b * w;

        ContactPoints::new(&[a, b])
    }

    /// Constructs a polytype from a simplex.
    /// Currently only implemented for Triangle and Tetrahedron simplex
    pub fn from_simplex<F: Fn(&[SupportPoint], &[u16]) -> Face>(
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
