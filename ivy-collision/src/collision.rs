use glam::{Mat4, Vec3};
use ivy_core::{
    gizmos::{self, DrawGizmos, GizmosSection, DEFAULT_RADIUS, DEFAULT_THICKNESS},
    Color, ColorExt,
};
use ordered_float::OrderedFloat;
use slotmap::Key;

use crate::{
    body::{BodyIndex, ContactIndex},
    epa, gjk,
    util::minkowski_diff,
    EntityPayload, Shape,
};

#[derive(Debug, Clone)]
pub struct Contact {
    /// The closest points on the two colliders, respectively
    pub point_a: Vec3,
    pub point_b: Vec3,
    pub depth: f32,
    pub normal: Vec3,
}

impl DrawGizmos for Contact {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        gizmos.draw(gizmos::Sphere {
            origin: self.point_a,
            color: Color::green(),
            ..Default::default()
        });
        gizmos.draw(gizmos::Sphere {
            origin: self.point_b,
            color: Color::green(),
            ..Default::default()
        });
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PersistentContactPoint {
    pos: Vec3,
    global_anchors: (Vec3, Vec3),
    local_anchors: (Vec3, Vec3),
    pub normal_impulse: f32,
    pub tangent_impulse: f32,
    pub tangent: Vec3,
    depth: f32,
    normal: Vec3,
}

impl PersistentContactPoint {
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

impl DrawGizmos for PersistentContactPoint {
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

/// A stable intersection of 1..=4 points
#[derive(Clone)]
pub struct PersistentContact {
    pub a: EntityPayload,
    pub b: EntityPayload,
    // pub surface: ContactSurface,
    pub points: Vec<PersistentContactPoint>,

    // island links
    pub island: BodyIndex,
    pub next_contact: ContactIndex,
    pub prev_contact: ContactIndex,
    pub generation: u32,
}

const CONTACT_THRESHOLD: f32 = 0.1;
impl PersistentContact {
    pub fn new(
        a: EntityPayload,
        b: EntityPayload,
        // surface: ContactSurface,
        point: PersistentContactPoint,
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

    pub fn add_point(&mut self, point: PersistentContactPoint) {
        for cur_point in &mut self.points {
            if cur_point.pos.distance_squared(point.pos) < CONTACT_THRESHOLD * CONTACT_THRESHOLD {
                *cur_point = PersistentContactPoint {
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

    fn assemble_point(&mut self, point: PersistentContactPoint) {
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

    pub fn points(&self) -> &[PersistentContactPoint] {
        &self.points
    }

    pub fn points_mut(&mut self) -> &mut Vec<PersistentContactPoint> {
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

impl DrawGizmos for PersistentContact {
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

impl std::fmt::Debug for PersistentContact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Contact")
            .field("next_contact", &self.next_contact)
            .field("prev_contact", &self.next_contact)
            .field("island", &self.island)
            .finish()
    }
}

/// Contains temporary state to accelerate contact generation
pub struct IntersectionGenerator {}

impl IntersectionGenerator {
    pub fn new() -> Self {
        Self {}
    }

    pub fn intersect(&mut self, a: &impl Shape, b: &impl Shape) -> Option<Contact> {
        let (intersect, simplex) = gjk(a, b);

        if !intersect {
            return None;
        }

        let contact_info = epa(simplex, |dir| minkowski_diff(a, b, dir));

        Some(contact_info)
    }
}

impl Default for IntersectionGenerator {
    fn default() -> Self {
        Self::new()
    }
}
