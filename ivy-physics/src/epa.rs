use smallvec::{Array, SmallVec};
use ultraviolet::{Mat4, Vec3, Vec4};

use crate::{
    collision::{minkowski_diff, CollisionPrimitive, TOLERANCE},
    gjk::Simplex,
};

pub struct Intersection {
    pub depth: f32,
    pub normal: Vec3,
}

#[derive(Clone, Copy, Debug)]
struct SignedNormal {
    normal: Vec3,
    distance: f32,
}

#[derive(Clone, Copy, Debug)]
struct Face {
    indices: [u16; 3],
    normal: Vec4,
}

impl Face {
    pub fn new(points: &[Vec3], indices: &[u16]) -> Self {
        let [a, b, c] = [
            points[indices[0] as usize],
            points[indices[1] as usize],
            points[indices[2] as usize],
        ];

        let normal = (b - a).cross(c - a).normalized();

        // Distance to the origin
        let distance = normal.dot(a);

        // Take care of handedness
        let normal = normal * distance.signum();
        let distance = distance.abs();

        Face {
            indices: [indices[0], indices[1], indices[2]],
            normal: Vec4::new(normal.x, normal.y, normal.z, distance),
        }
    }
}

type Edge = (u16, u16);

#[derive(Debug)]
struct Polytype {
    points: SmallVec<[Vec3; 8]>,
    // Normals and distances combined
    faces: SmallVec<[Face; 16]>,
}

impl Polytype {
    pub fn new(points: &[Vec3], faces: &[u16]) -> Self {
        let faces = faces
            .chunks_exact(3)
            .map(|val| Face::new(points, val))
            .collect();

        Self {
            points: points.into(),
            faces,
        }
    }

    fn find_closest_face(&self) -> Option<(u16, Face)> {
        self.faces
            .iter()
            .enumerate()
            .min_by(|a, b| a.1.normal.w.partial_cmp(&b.1.normal.w).unwrap())
            .map(|(a, b)| (a as u16, *b))
    }

    pub fn add(&mut self, p: Vec3) {
        // remove faces that can see the point
        let mut edges = SmallVec::<[Edge; 16]>::new();
        let mut i = 0;
        while i < self.faces.len() {
            let dot = self.faces[i]
                .normal
                .xyz()
                .dot(p - self.points[self.faces[i].indices[0] as usize]);
            if dot > 0.0 {
                let face = self.faces.swap_remove(i);
                remove_or_add_edge(&mut edges, (face.indices[0], face.indices[1]));
                remove_or_add_edge(&mut edges, (face.indices[1], face.indices[2]));
                remove_or_add_edge(&mut edges, (face.indices[2], face.indices[0]));
            } else {
                i += 1;
            }
        }

        assert_ne!(self.faces.len(), 0);

        // add vertex
        let n = self.points.len();
        self.points.push(p);
        let points = &self.points;

        // add new faces
        let new_faces = edges
            .into_iter()
            .map(|(a, b)| Face::new(points, &[n as _, a, b]));

        self.faces.extend(new_faces);
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

pub fn epa<A: CollisionPrimitive, B: CollisionPrimitive>(
    a_transform: &Mat4,
    b_transform: &Mat4,
    a_transform_inv: &Mat4,
    b_transform_inv: &Mat4,
    a_coll: &A,
    b_coll: &B,
    simplex: Simplex,
) -> Intersection {
    // simplex.force_tetrahedron(
    //     a_transform,
    //     b_transform,
    //     a_transform_inv,
    //     b_transform_inv,
    //     a_coll,
    //     b_coll,
    // );

    eprintln!("Simplex: {:?}", simplex.points());

    let mut polytype = Polytype::new(simplex.points(), &[0, 1, 2, 0, 3, 1, 0, 2, 3, 1, 3, 2]);

    let mut iterations = 0;
    loop {
        let (_, min) = polytype.find_closest_face().unwrap();

        // assert_eq!(min.normal.mag(), 1.0);

        let support = minkowski_diff(
            a_transform,
            b_transform,
            a_transform_inv,
            b_transform_inv,
            a_coll,
            b_coll,
            min.normal.xyz(),
        );

        let support_dist = min.normal.xyz().dot(support);

        if (support_dist - min.normal.w) <= TOLERANCE || iterations > 10 {
            return Intersection {
                depth: min.normal.w,
                normal: min.normal.xyz(),
            };
        }
        // Support is further than the current closest normal
        else {
            polytype.add(support)
        }

        iterations += 1;
    }
}
