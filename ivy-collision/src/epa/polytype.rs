use glam::Vec3;
use ivy_core::{
    gizmos::{self, DrawGizmos, GizmosSection, Line, DEFAULT_RADIUS},
    Color, ColorExt,
};
use ordered_float::OrderedFloat;
use smallvec::{Array, SmallVec};
use std::ops::Index;

use crate::{
    util::{barycentric_vector, ray_distance, SupportPoint},
    Ray, Simplex,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct PolytypeFace {
    pub indices: [u16; 3],
    pub normal: Vec3,
    pub distance: f32,
}

impl PolytypeFace {
    /// Returns None if the normal cannot be calculated
    pub fn new(points: &[SupportPoint], indices: [u16; 3]) -> Self {
        let [p1, p2, p3] = [
            points[indices[0] as usize],
            points[indices[1] as usize],
            points[indices[2] as usize],
        ];

        let normal = (p2.p - p1.p).cross(p3.p - p1.p).normalize();

        if !normal.is_finite() {
            tracing::warn!("normal: {normal} {p1} {p2} {p3}");
        }

        // assert!(normal.is_finite());

        // Distance to the origin of the minkowski difference
        let distance = normal.dot(p1.p);

        // assert!(distance.is_finite());
        // Take care of handedness
        let normal = normal * distance.signum();
        let distance = distance.abs();

        PolytypeFace {
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

        let total_radial: Vec3 = [p1, p2]
            .iter()
            .map(|p| p.p - reference_point)
            .fold(Vec3::ZERO, |acc, val| acc + val);

        let normal = (p2.p - p1.p).cross(p3.p - p1.p).normalize();

        let normal = normal * normal.dot(total_radial).signum();

        let distance = ray_distance(p1, normal, ray);

        PolytypeFace {
            indices: [indices[0], indices[1], indices[2]],
            normal,
            distance,
        }
    }

    pub fn edges(&self) -> [Edge; 3] {
        [
            (self.indices[0], self.indices[1]),
            (self.indices[1], self.indices[2]),
            (self.indices[2], self.indices[0]),
        ]
    }
}

type Edge = (u16, u16);

#[derive(Debug, Clone)]
pub struct Polytype {
    pub points: SmallVec<[SupportPoint; 32]>,
    // Normals and distances combined
    pub faces: SmallVec<[PolytypeFace; 32]>,
}

impl Polytype {
    pub fn new<F: Fn(&[SupportPoint], [u16; 3]) -> PolytypeFace>(
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

    pub fn find_furthest_face(&self) -> Option<(u16, PolytypeFace)> {
        self.faces
            .iter()
            .enumerate()
            .max_by_key(|val| OrderedFloat(val.1.distance))
            .map(|(a, b)| (a as u16, *b))
    }

    pub fn find_closest_face(&self) -> Option<(u16, PolytypeFace)> {
        self.faces
            .iter()
            .enumerate()
            .filter(|v| v.1.normal.is_finite())
            .min_by_key(|v| ordered_float::NotNan::new(v.1.distance).unwrap())
            .map(|(a, b)| (a as u16, *b))
    }

    // Adds a point to the polytype
    pub fn add_point<F: Fn(&[SupportPoint], [u16; 3]) -> PolytypeFace>(
        &mut self,
        p: SupportPoint,
        face_func: F,
    ) {
        // remove faces that can see the point
        let mut edges = SmallVec::<[Edge; 16]>::new();
        let points = &self.points;

        self.faces.retain(|face| {
            let to_support = p.p - points[face.indices[0] as usize].p;
            if face.normal.dot(p.p) > face.normal.dot(self.points[face.indices[0] as usize].p) {
                face.edges().iter().for_each(|edge| {
                    add_if_unique(&mut edges, *edge);
                });
                false
            } else {
                true
            }
        });

        // add vertex
        let new_index = self.points.len();
        self.points.push(p);
        let points = &self.points;

        // add new faces
        let new_faces = edges
            .into_iter()
            .map(|(a, b)| face_func(points, [a, b, new_index as u16]));

        self.faces.extend(new_faces);
        assert_ne!(self.faces.len(), 0);
    }

    // Adds a point to the polytype onto the specified face.
    // Polytype will not have any back faces
    pub fn add_decimate(&mut self, face: PolytypeFace, p: SupportPoint, ray: &Ray) {
        // add vertex
        let n = self.points.len() as u16;
        self.points.push(p);
        let points = &self.points;

        let new_faces = [(0, 1, 2), (0, 2, 1), (1, 2, 0)].iter().map(|val| {
            PolytypeFace::new_ray(
                points,
                [face.indices[val.0], face.indices[val.1], n],
                ray,
                points[face.indices[val.2] as usize].p,
            )
        });

        self.faces.clear();
        self.faces.extend(new_faces);
        assert_eq!(self.faces.len(), 3);
    }

    pub(crate) fn contact_points(&self, face: PolytypeFace) -> (Vec3, Vec3) {
        let [p1, p2, p3] = [
            self[face.indices[0]],
            self[face.indices[1]],
            self[face.indices[2]],
        ];

        let (u, v, w) = barycentric_vector(face.normal * face.distance, p1.p, p2.p, p3.p);

        let contact_a = p1.a * u + p2.a * v + p3.a * w;

        let contact_b = p1.b * u + p2.b * v + p3.b * w;

        (contact_a, contact_b)
    }

    /// Constructs a polytype from a simplex.
    /// Currently only implemented for Triangle and Tetrahedron simplex
    pub(crate) fn from_simplex<F: Fn(&[SupportPoint], [u16; 3]) -> PolytypeFace>(
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

impl DrawGizmos for Polytype {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        for (i, point) in self.points.iter().enumerate() {
            gizmos.draw(gizmos::Sphere::new(
                point.p,
                DEFAULT_RADIUS,
                Color::from_hsla(i as f32 * 60.0, 1.0, 0.5, 1.0),
            ));
        }

        for face in &self.faces {
            let color = Color::blue();

            let p1 = self.points[face.indices[0] as usize].p;
            let p2 = self.points[face.indices[1] as usize].p;
            let p3 = self.points[face.indices[2] as usize].p;

            let midpoint = (p1 + p2 + p3) / 3.0;
            gizmos.draw(Line::new(midpoint, face.normal * 0.2, 0.002, color));
            gizmos.draw(gizmos::Sphere::new(midpoint, 0.01, color));

            let indent = |p: Vec3| p - (p - midpoint).reject_from(face.normal).normalize() * 0.0;

            let p1 = indent(p1);
            let p2 = indent(p2);
            let p3 = indent(p3);
            for edge in [(p1, p2), (p2, p3), (p3, p1)] {
                gizmos.draw(Line::from_points(edge.0, edge.1, 0.002, color))
            }
        }
    }
}

impl Index<u16> for Polytype {
    type Output = SupportPoint;

    fn index(&self, index: u16) -> &Self::Output {
        &self.points[index as usize]
    }
}

fn add_if_unique<T: Array<Item = Edge>>(edges: &mut SmallVec<T>, edge: Edge) -> bool {
    if let Some(index) = edges.iter().position(|val| {
        // assert_ne!(**val, edge);
        (val.0, val.1) == (edge.1, edge.0)
    }) {
        edges.remove(index);
        false
    } else {
        edges.push(edge);
        true
    }
}
