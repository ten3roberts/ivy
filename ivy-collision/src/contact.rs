use std::{fmt::Display, mem};

use glam::{vec2, Vec2, Vec3};
use itertools::Itertools;
use ivy_core::{
    gizmos::{self, DrawGizmos, GizmosSection, Polygon, DEFAULT_RADIUS, DEFAULT_THICKNESS},
    Color, ColorExt,
};
use ordered_float::Float;
use palette::{
    cast::into_uint_ref,
    num::{Abs, Signum},
};

use crate::{util::TOLERANCE, Shape};

#[derive(Debug, Clone)]
pub struct ContactSurface {
    intersection: Vec<Vec3>,
    midpoint: Vec3,
    normal: Vec3,
    depth: f32,
    b_surface: Vec<Vec3>,
    a_surface: Vec<Vec3>,
    area: f32,
}

impl Display for ContactSurface {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ContactSurface")
            .field("midpoint", &self.midpoint)
            .field("normal", &self.normal)
            .field("depth", &self.depth)
            .finish()
    }
}

impl ContactSurface {
    pub fn midpoint(&self) -> Vec3 {
        self.midpoint
    }

    pub fn normal(&self) -> Vec3 {
        self.normal
    }

    pub fn intersection(&self) -> &[Vec3] {
        &self.intersection
    }

    pub fn depth(&self) -> f32 {
        self.depth
    }

    pub fn area(&self) -> f32 {
        self.area
    }
}

pub struct ContactGenerator {
    a_surface: Vec<Vec3>,
    b_surface: Vec<Vec3>,
}

impl ContactGenerator {
    pub(crate) fn new() -> Self {
        Self {
            a_surface: Default::default(),
            b_surface: Default::default(),
        }
    }

    pub fn generate<A: Shape, B: Shape>(
        &mut self,
        a: A,
        b: B,
        normal: Vec3,
        contact_basis: Vec3,
        depth: f32,
    ) -> ContactSurface {
        debug_assert!(contact_basis.is_finite());
        let _span = tracing::info_span!("generate", ?normal).entered();
        let normal = normal.normalize();
        let a_surface = &mut self.a_surface;
        let b_surface = &mut self.b_surface;

        a_surface.clear();
        b_surface.clear();

        a.surface_contour(normal, a_surface);
        b.surface_contour(-normal, b_surface);

        assert!(!a_surface.is_empty());
        assert!(!b_surface.is_empty());

        debug_assert!(a_surface.iter().all(|v| v.is_finite()));
        debug_assert!(b_surface.iter().all(|v| v.is_finite()));

        let tan = if normal.dot(Vec3::X).abs() == 1.0 {
            Vec3::Y * normal.dot(Vec3::X).signum()
        } else {
            normal.cross(Vec3::X).normalize()
        };

        assert!(tan.is_normalized());

        const DISC_AREA: f32 = 0.4;
        const LINE_WIDTH: f32 = 0.2;

        let bitan = tan.cross(normal).normalize();

        let flatten = |v: &Vec3| vec2(v.dot(tan), v.dot(bitan));

        let to_world = |v: Vec2| v.x * tan + v.y * bitan + contact_basis.dot(normal) * normal;

        if a_surface.len() == 1 {
            tracing::info!("point a");
            let p = to_world(flatten(&a_surface[0]));

            return ContactSurface {
                intersection: vec![p],
                midpoint: p,
                normal,
                depth,
                b_surface: b_surface.clone(),
                a_surface: a_surface.clone(),
                area: DISC_AREA,
            };
        }

        if b_surface.len() == 1 {
            let p = to_world(flatten(&b_surface[0]));

            return ContactSurface {
                intersection: vec![p],
                midpoint: p,
                normal,
                depth,
                b_surface: b_surface.clone(),
                a_surface: a_surface.clone(),
                area: DISC_AREA,
            };
        }

        if let ([a1, a2], [b1, b2]) = (&a_surface[..], &b_surface[..]) {
            tracing::info!(?a1, ?a2, ?b1, ?b2, "line-line");
            let a1 = flatten(a1);
            let a2 = flatten(a2);
            let mut b1 = flatten(b1);
            let mut b2 = flatten(b2);

            let a_dir = (a2 - a1).normalize();
            let b_dir = b2 - b1;

            // Clip colinear line segments
            if a_dir.perp_dot(b_dir) < TOLERANCE {
                if a_dir.dot(b_dir) < 0.0 {
                    mem::swap(&mut b1, &mut b2);
                }

                let basis = a1.reject_from(a_dir);

                let a1 = a1.dot(a_dir);
                let a2 = a2.dot(a_dir);
                let b1 = b1.dot(a_dir);
                let b2 = b2.dot(a_dir);

                assert!(a1 < a2);

                let p1 = a1.max(b1);
                let p2 = a2.min(b2);

                let mid = (p1 + p2) / 2.0;
                let midpoint = to_world(mid * a_dir + basis);
                return ContactSurface {
                    intersection: vec![midpoint],
                    midpoint,
                    normal,
                    depth,
                    b_surface: b_surface.clone(),
                    a_surface: a_surface.clone(),
                    area: (p1 - p2).abs() * LINE_WIDTH,
                };
            }

            let p = clip_segment((b2 - b1).perp(), b2, a1, a2).unwrap();
            return ContactSurface {
                intersection: vec![to_world(p)],
                midpoint: to_world(p),
                normal,
                depth,
                b_surface: b_surface.clone(),
                a_surface: a_surface.clone(),
                area: DISC_AREA,
            };
        }

        let line_case = |p1, p2, surface: &[Vec3], winding| {
            tracing::info!(?p1, ?p2, ?surface, winding, "line surface");
            let [p1, p2] = clip_line_face(
                [flatten(&p1), flatten(&p2)],
                surface.iter().map(flatten),
                winding,
            );

            let midpoint = to_world((p1 + p2) / 2.0);

            tracing::info!(?p1, ?p2, ?midpoint);

            ContactSurface {
                intersection: vec![to_world(p1), to_world(p2)],
                midpoint,
                normal,
                depth,
                b_surface: b_surface.clone(),
                a_surface: a_surface.clone(),
                area: p1.distance(p2) * LINE_WIDTH,
            }
        };

        if let [p1, p2] = a_surface[..] {
            return line_case(p1, p2, b_surface, 1.0);
        }

        if let [p1, p2] = b_surface[..] {
            return line_case(p1, p2, a_surface, -1.0);
        }

        tracing::info!("face-face");
        let mut input = a_surface.iter().map(flatten).collect_vec();
        let mut output = Vec::new();
        mem::swap(&mut input, &mut output);

        for (b1, b2) in b_surface.iter().map(flatten).circular_tuple_windows() {
            mem::swap(&mut input, &mut output);
            output.clear();

            let clip_dir = b2 - b1;
            let clip_edge = clip_dir.perp().normalize();

            for (&prev_point, &current_point) in input.iter().circular_tuple_windows() {
                let intersect = clip_segment(clip_edge, b2, prev_point, current_point);

                // output.push(a2);
                // current point is inside
                let current_dot = (b2 - current_point).dot(clip_edge);
                let prev_dot = (b2 - prev_point).dot(clip_edge);

                assert!(current_dot.is_finite());
                assert!(prev_dot.is_finite());
                // tracing::info!(current_dot, prev_dot);
                if current_dot < TOLERANCE {
                    if prev_dot > TOLERANCE {
                        output.push(intersect.unwrap())
                    }
                    output.push(current_point);
                } else if prev_dot < -TOLERANCE {
                    output.push(intersect.unwrap())
                }
            }

            // tracing::info!(output = output.len());
        }

        let mut midpoint = output.iter().sum::<Vec2>() / output.len() as f32;

        assert!(
            midpoint.is_finite(),
            "{a_surface:?} {b_surface:?} {output:?}"
        );

        ContactSurface {
            area: polygon_area(&output, midpoint),
            b_surface: b_surface.clone(),
            a_surface: a_surface.clone(),
            intersection: output.into_iter().map(to_world).collect_vec(),
            midpoint: to_world(midpoint),
            normal,
            depth,
        }
    }
}

// maybe an approximation could work instead :P
fn polygon_area(points: &[Vec2], midpoint: Vec2) -> f32 {
    let c = midpoint;
    let area: f32 = points
        .iter()
        .circular_tuple_windows()
        .map(|(a, b)| (a.x * (b.y - c.y) + b.x * (c.y - a.y) + c.x * (a.y - b.y)).abs())
        .sum();

    area / 2.0
}

impl Default for ContactGenerator {
    fn default() -> Self {
        Self::new()
    }
}

fn clip_line_face(
    line: [Vec2; 2],
    face: impl ExactSizeIterator<Item = Vec2> + Clone,
    winding: f32,
) -> [Vec2; 2] {
    let [mut a, mut b] = line;
    for (e1, e2) in face.circular_tuple_windows() {
        if a.distance(b) < TOLERANCE {
            break;
        }

        let clip_edge = (e2 - e1).perp().normalize() * winding;

        let intersection = clip_segment(clip_edge, e2, a, b);

        let a_dot = (e2 - a).dot(clip_edge);
        let b_dot = (e2 - b).dot(clip_edge);

        // tracing::info!(a_dot, b_dot, ?intersection);

        // if a is outside, clip
        if a_dot > 0.0 {
            a = intersection.unwrap();
        }

        // if b is outside, clip
        if b_dot > 0.0 {
            b = intersection.unwrap();
        }
    }

    [a, b]
}

fn clip_segment(normal: Vec2, point: Vec2, start: Vec2, end: Vec2) -> Option<Vec2> {
    let num = (point - start).dot(normal);
    let segment_dir = end - start;
    let denom = normal.dot(segment_dir);

    let t = num / denom;

    // if !(-TOLERANCE..=1.0 + TOLERANCE).contains(&t) {
    //     return None;
    // }

    let dot = normal.dot(segment_dir);

    // colinear
    if dot.abs() <= 0.0 {
        return None;
    }

    let p = start + t * segment_dir;
    Some(p)
}

impl DrawGizmos for ContactSurface {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        gizmos.draw(Polygon::new(self.intersection.iter().copied()).with_color(Color::blue()));
        gizmos.draw(Polygon::new(self.a_surface.iter().copied()).with_color(Color::green()));
        gizmos.draw(Polygon::new(self.b_surface.iter().copied()).with_color(Color::red()));

        gizmos.draw(gizmos::Sphere::new(
            self.midpoint,
            DEFAULT_RADIUS,
            Color::cyan(),
        ));

        gizmos.draw(gizmos::Sphere::new(
            self.midpoint - self.normal * self.depth * 0.5,
            DEFAULT_RADIUS,
            Color::red(),
        ));

        gizmos.draw(gizmos::Sphere::new(
            self.midpoint + self.normal * self.depth * 0.5,
            DEFAULT_RADIUS,
            Color::red(),
        ));

        gizmos.draw(gizmos::Line::new(
            self.midpoint,
            self.normal * 0.1,
            DEFAULT_THICKNESS,
            Color::blue(),
        ));
    }
}
