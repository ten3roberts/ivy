use std::ops::Index;

use glam::{Mat4, Vec3};
use ivy_core::{
    gizmos::{self, DrawGizmos, GizmosSection, Line, DEFAULT_RADIUS, DEFAULT_THICKNESS},
    Color, ColorExt,
};
use ordered_float::OrderedFloat;
use slotmap::Key;

use crate::{
    body::{BodyIndex, ContactIndex},
    contact::{ContactGenerator, ContactSurface},
    epa, gjk,
    util::minkowski_diff,
    EntityPayload, Shape,
};

/// Contains temporary state to accelerate contact generation
pub struct IntersectionGenerator {
    contact_generator: ContactGenerator,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ContactPoints {
    Single([Vec3; 1]),
    Double([Vec3; 2]),
}

impl ContactPoints {
    pub fn single(p: Vec3) -> Self {
        Self::Single([p])
    }

    pub fn double(a: Vec3, b: Vec3) -> Self {
        Self::Double([a, b])
    }

    pub fn points(&self) -> &[Vec3] {
        match self {
            ContactPoints::Single(val) => val,
            ContactPoints::Double(val) => val,
        }
    }

    pub fn iter(&self) -> std::slice::Iter<Vec3> {
        self.into_iter()
    }

    pub fn reverse(&self) -> Self {
        match *self {
            Self::Single(p) => Self::Single(p),
            Self::Double([a, b]) => Self::Double([b, a]),
        }
    }
}

impl DrawGizmos for ContactPoints {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        for &p in self.iter() {
            gizmos.draw(gizmos::Sphere {
                origin: p,
                color: Color::green(),
                ..Default::default()
            })
        }
    }
}

impl From<Vec3> for ContactPoints {
    fn from(val: Vec3) -> Self {
        Self::Single([val])
    }
}

impl From<[Vec3; 1]> for ContactPoints {
    fn from(val: [Vec3; 1]) -> Self {
        Self::Single(val)
    }
}

impl From<[Vec3; 2]> for ContactPoints {
    fn from(val: [Vec3; 2]) -> Self {
        Self::Double(val)
    }
}

impl<'a> IntoIterator for &'a ContactPoints {
    type Item = &'a Vec3;

    type IntoIter = std::slice::Iter<'a, Vec3>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            ContactPoints::Single(val) => val.iter(),
            ContactPoints::Double(val) => val.iter(),
        }
    }
}

impl Index<usize> for ContactPoints {
    type Output = Vec3;

    fn index(&self, index: usize) -> &Self::Output {
        &self.points()[index]
    }
}

#[derive(Debug, Clone)]
pub struct Intersection {
    /// The closest points on the two colliders, respectively
    pub points: ContactPoints,
    pub depth: f32,
    pub normal: Vec3,
    pub polytype: epa::Polytype,
}

impl DrawGizmos for Intersection {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        // gizmos.draw(self.points);

        gizmos.draw(Line {
            origin: self.points[0],
            dir: self.normal * 0.2,
            color: Color::blue(),
            ..Default::default()
        });

        self.polytype.draw_primitives(gizmos);
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ContactPoint {
    pos: Vec3,
    global_anchors: (Vec3, Vec3),
    local_anchors: (Vec3, Vec3),
    pub normal_impulse: f32,
    pub tangent_impulse: f32,
    pub tangent: Vec3,
    depth: f32,
    normal: Vec3,
}

impl ContactPoint {
    pub fn new(
        global_anchors: (Vec3, Vec3),
        local_anchors: (Vec3, Vec3),
        depth: f32,
        normal: Vec3,
    ) -> Self {
        Self {
            pos: (global_anchors.0 + global_anchors.1) / 2.0,
            global_anchors,
            local_anchors,
            normal_impulse: 0.0,
            tangent_impulse: 0.0,
            tangent: Vec3::ZERO,
            depth,
            normal,
        }
    }

    pub fn pos(&self) -> Vec3 {
        self.pos
    }

    pub fn local_pos(&self) -> (Vec3, Vec3) {
        self.local_anchors
    }

    pub fn depth(&self) -> f32 {
        self.depth
    }

    pub fn normal(&self) -> Vec3 {
        self.normal
    }
}

impl DrawGizmos for ContactPoint {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        gizmos.draw(gizmos::Sphere::new(self.pos, DEFAULT_RADIUS, Color::blue()));
        gizmos.draw(gizmos::Line::new(
            self.pos,
            self.normal * (self.normal_impulse + 1.0).log10(),
            DEFAULT_THICKNESS,
            Color::green(),
        ));
    }
}

/// Represents a collision between two entities.
#[derive(Clone)]
pub struct Contact {
    pub a: EntityPayload,
    pub b: EntityPayload,
    // pub surface: ContactSurface,
    pub points: Vec<ContactPoint>,

    // island links
    pub island: BodyIndex,
    pub next_contact: ContactIndex,
    pub prev_contact: ContactIndex,
    pub generation: u32,
}

const CONTACT_THRESHOLD: f32 = 0.1;
impl Contact {
    pub fn new(
        a: EntityPayload,
        b: EntityPayload,
        // surface: ContactSurface,
        point: ContactPoint,
        generation: u32,
    ) -> Self {
        Self {
            a,
            b,
            // surface,
            points: vec![point],
            island: BodyIndex::null(),
            next_contact: ContactIndex::null(),
            prev_contact: ContactIndex::null(),
            generation,
        }
    }

    pub fn add_point(&mut self, point: ContactPoint) {
        for cur_point in &mut self.points {
            if cur_point.pos.distance_squared(point.pos) < CONTACT_THRESHOLD * CONTACT_THRESHOLD {
                *cur_point = ContactPoint {
                    normal_impulse: cur_point.normal_impulse,
                    tangent_impulse: cur_point.tangent_impulse,
                    tangent: cur_point.tangent,
                    ..point
                };
                return;
            }
        }

        self.assemble_point(point);
    }

    pub fn remove_invalid_points(&mut self, a: Mat4, b: Mat4) {
        self.points.retain(|v| {
            let a_pos = a.transform_point3(v.local_anchors.0);
            let b_pos = b.transform_point3(v.local_anchors.1);

            // (a_pos - v.global_anchors.0).dot(v.normal) >= -CONTACT_THRESHOLD
            //     && -(b_pos - v.global_anchors.1).dot(v.normal) >= -CONTACT_THRESHOLD
            a_pos.distance_squared(v.global_anchors.0) < CONTACT_THRESHOLD * CONTACT_THRESHOLD
                && b_pos.distance_squared(v.global_anchors.1)
                    < CONTACT_THRESHOLD * CONTACT_THRESHOLD
        });
    }

    fn assemble_point(&mut self, point: ContactPoint) {
        let &p1 = self
            .points
            .iter()
            .chain([&point])
            .max_by_key(|v| OrderedFloat(v.depth))
            .unwrap();

        let Some(&p2) = self
            .points
            .iter()
            .chain([&point])
            .max_by_key(|v| OrderedFloat(v.pos.distance(p1.pos)))
        else {
            self.points = vec![p1];
            return;
        };

        if p1.pos == p2.pos {
            self.points = vec![p1];
            return;
        }

        fn distance_from_line(l1: Vec3, l2: Vec3, v: Vec3) -> f32 {
            ((l2.y - l1.y) * v.x - (l2.x - l1.x) * v.y + (l2.x * l1.y) - (l2.y * l1.x)).abs()
                / (l1 - l2).length()
        }

        let l1 = p1.pos;
        let l2 = p2.pos;

        let Some((&p3, p3_dist)) = self
            .points
            .iter()
            .chain([&point])
            .map(|v| (v, distance_from_line(l1, l2, v.pos)))
            .max_by_key(|v| OrderedFloat(v.1))
        else {
            self.points = vec![p1, p2];
            return;
        };

        if p3_dist == 0.0 {
            self.points = vec![p1, p2];
            return;
        }

        let triangle = [
            TriangleSide::new(p1.pos, p2.pos, p3.pos),
            TriangleSide::new(p2.pos, p3.pos, p1.pos),
            TriangleSide::new(p3.pos, p1.pos, p2.pos),
        ];

        fn distance_from_triangle(triangle: &[TriangleSide; 3], p: Vec3) -> f32 {
            *triangle
                .iter()
                .map(|v| OrderedFloat(v.distance_to_point(p)))
                .max()
                .unwrap()
        }

        let Some((&p4, p4_dist)) = self
            .points
            .iter()
            .chain([&point])
            .map(|v| (v, distance_from_triangle(&triangle, v.pos)))
            .max_by_key(|v| OrderedFloat(v.1))
        else {
            self.points = vec![p1, p2, p3];
            return;
        };

        // inside triangle
        if p4_dist <= 0.0 {
            self.points = vec![p1, p2, p3];
            return;
        }

        self.points = vec![p1, p2, p3, p4];
    }

    pub fn points(&self) -> &[ContactPoint] {
        &self.points
    }

    pub fn points_mut(&mut self) -> &mut Vec<ContactPoint> {
        &mut self.points
    }

    pub fn depth(&self) -> f32 {
        self.points
            .iter()
            .map(|v| v.depth)
            .max_by_key(|&v| OrderedFloat(v))
            .unwrap_or_default()
    }
}

impl DrawGizmos for Contact {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        for p in &self.points {
            p.draw_primitives(gizmos);
        }
    }
}

struct TriangleSide {
    p1: Vec3,
    normal: Vec3,
}

impl TriangleSide {
    pub fn new(p1: Vec3, p2: Vec3, p_opposite: Vec3) -> Self {
        let dir = (p2 - p1).normalize();
        let normal = dir.cross(p1 - p_opposite).cross(dir).normalize();

        TriangleSide { p1, normal }
    }

    pub fn distance_to_point(&self, p: Vec3) -> f32 {
        (p - self.p1).dot(self.normal)
    }
}

impl std::fmt::Debug for Contact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Contact")
            .field("next_contact", &self.next_contact)
            .field("prev_contact", &self.next_contact)
            .field("island", &self.island)
            .finish()
    }
}

impl IntersectionGenerator {
    pub fn new() -> Self {
        Self {
            contact_generator: ContactGenerator::new(),
        }
    }

    pub fn test_intersect(&mut self, a: &impl Shape, b: &impl Shape) -> Option<Intersection> {
        let (intersect, simplex) = gjk(a, b);

        if !intersect {
            return None;
        }

        let contact_info = epa(simplex, |dir| minkowski_diff(a, b, dir));

        Some(contact_info)
    }

    /// Returns the intersection of two shapes
    pub fn intersect<A: Shape, B: Shape>(&mut self, a: &A, b: &B) -> Option<ContactSurface> {
        let (intersect, simplex) = gjk(a, b);

        if !intersect {
            return None;
        }

        let contact_info = epa(simplex, |dir| minkowski_diff(a, b, dir));

        let surface = self.contact_generator.generate(
            a,
            b,
            contact_info.normal,
            contact_info.points.points().iter().sum::<Vec3>()
                / contact_info.points.points().len() as f32,
            contact_info.depth,
        );

        Some(surface)
    }
}

impl Default for IntersectionGenerator {
    fn default() -> Self {
        Self::new()
    }
}
