use std::mem;

use glam::{vec2, Vec2, Vec3};
use itertools::Itertools;
use ivy_core::{gizmos, Color, ColorExt, DrawGizmos, DEFAULT_RADIUS, DEFAULT_THICKNESS};
use ordered_float::Float;
use palette::{
    cast::{into_uint_ref, try_from_component_vec},
    num::Abs,
};

use crate::{util::TOLERANCE, Shape};

#[derive(Debug, Clone)]
pub struct ContactSurface {
    intersection: Vec<Vec3>,
    midpoint: Vec3,
    normal: Vec3,
}

pub fn generate_contact_surface<A: Shape, B: Shape>(
    a: A,
    b: B,
    normal: Vec3,
    contact_basis: Vec3,
) -> ContactSurface {
    let mut a_surface = Vec::new();
    let mut b_surface = Vec::new();

    a.clipping_surface(normal, &mut a_surface);
    b.clipping_surface(-normal, &mut b_surface);

    if a_surface.len() == 1 {
        return ContactSurface {
            intersection: a_surface[0..1].to_vec(),
            midpoint: a_surface[0],
            normal,
        };
    }

    if b_surface.len() == 1 {
        return ContactSurface {
            intersection: b_surface[0..1].to_vec(),
            midpoint: b_surface[0],
            normal,
        };
    }

    let tan = if normal.dot(Vec3::X).abs() > 1.0 - TOLERANCE {
        Vec3::Y
    } else {
        normal.cross(Vec3::X)
    };

    let bitan = tan.cross(normal);

    let flatten = |v: &Vec3| vec2(v.dot(tan), v.dot(bitan));

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
            let current_dot = (b1 - current_point).dot(clip_edge);
            let prev_dot = (b1 - prev_point).dot(clip_edge);

            tracing::info!(current_dot, prev_dot, %clip_edge);

            if current_dot <= 0.0 {
                if prev_dot > 0.0 {
                    output.push(intersect.unwrap())
                }
                output.push(current_point);
            } else if prev_dot <= 0.0 {
                output.push(intersect.unwrap())
            }
        }
    }

    let midpoint = output.iter().sum::<Vec2>() / output.len() as f32;

    let to_world = |v: Vec2| v.x * tan + v.y * bitan - contact_basis * normal;

    ContactSurface {
        intersection: output.into_iter().map(|v| to_world(v)).collect_vec(),
        midpoint: to_world(midpoint),
        normal,
    }
}

fn clip_segment(normal: Vec2, point: Vec2, start: Vec2, end: Vec2) -> Option<Vec2> {
    let num = (point - start).dot(normal);
    let segment_dir = end - start;
    let denom = normal.dot(segment_dir);

    let t = num / denom;

    if !(-TOLERANCE..=1.0 + TOLERANCE).contains(&t) {
        tracing::info!(t, num, denom, "outside");
        return None;
    }

    let dot = normal.dot(segment_dir);

    // colinear
    if dot.abs() < TOLERANCE {
        tracing::info!(dot, "colinear");
        return None;
    }

    let p = start + t * segment_dir;
    Some(p)
}

impl DrawGizmos for ContactSurface {
    fn draw_primitives(&self, gizmos: &mut ivy_core::GizmosSection) {
        for (&p1, &p2) in self.intersection.iter().circular_tuple_windows() {
            gizmos.draw(gizmos::Sphere::new(p1, DEFAULT_RADIUS, Color::red()));
            gizmos.draw(gizmos::Line::from_points(
                p1,
                p2,
                DEFAULT_THICKNESS,
                Color::cyan(),
            ));
        }

        gizmos.draw(gizmos::Sphere::new(
            self.midpoint,
            DEFAULT_RADIUS,
            Color::purple(),
        ));
        gizmos.draw(gizmos::Line::new(
            self.midpoint,
            self.normal * 0.2,
            DEFAULT_THICKNESS,
            Color::blue(),
        ));
    }
}
