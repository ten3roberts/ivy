use std::f32::consts::TAU;

use glam::{vec3, Mat4, Vec3};
use palette::Srgba;

use crate::{math::Axis3D, Color, ColorExt, DEG_45};

use super::{DrawGizmos, GizmoVertex, GizmosSection, SphereGizmo};

pub struct TranslateGizmo {
    pub transform: Mat4,
}

impl TranslateGizmo {
    pub fn new(transform: Mat4) -> Self {
        Self { transform }
    }
}

impl DrawGizmos for TranslateGizmo {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        let axes = [
            (Axis3D::X, Color::red()),
            (Axis3D::Y, Color::green()),
            (Axis3D::Z, Color::blue()),
        ];

        let position = self.transform.transform_point3(Vec3::ZERO);

        for (axis, color) in axes.iter() {
            let direction = self.transform.transform_vector3(axis.to_vec3()).normalize();

            gizmos.draw(ArrowGizmo::new(position, direction).with_color(*color))
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrowGizmo {
    position: Vec3,
    head_length: f32,
    head_radius: f32,
    tail_radius: f32,
    segments: u32,
    direction: Vec3,
    color: Srgba,
}

impl ArrowGizmo {
    pub fn new(position: Vec3, direction: Vec3) -> Self {
        Self {
            head_length: 0.15,
            head_radius: 0.05,
            tail_radius: 0.01,
            segments: 12,
            direction,
            color: Srgba::new(1.0, 0.0, 0.0, 1.0),
            position,
        }
    }

    pub fn with_color(mut self, color: Srgba) -> Self {
        self.color = color;
        self
    }

    pub fn with_head_length(mut self, length: f32) -> Self {
        self.head_length = length;
        self
    }

    pub fn with_head_radius(mut self, radius: f32) -> Self {
        self.head_radius = radius;
        self
    }

    pub fn with_tail_radius(mut self, radius: f32) -> Self {
        self.tail_radius = radius;
        self
    }

    pub fn with_segments(mut self, segments: u32) -> Self {
        self.segments = segments;
        self
    }
}

impl DrawGizmos for ArrowGizmo {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let tan = if self.direction.distance(Vec3::Y) < 0.001 {
            Vec3::Z
        } else {
            self.direction.cross(Vec3::Y).normalize()
        };

        let length = self.direction.length();
        let normalized_direction = self.direction.normalize();

        let tip = self.position + normalized_direction * length;
        vertices.push(GizmoVertex::new(tip, self.color));

        let bitan = normalized_direction.cross(tan);

        for theta in 0..self.segments {
            let theta = theta as f32 * TAU / self.segments as f32 + DEG_45;

            let x = theta.cos() * self.head_radius;
            let y = theta.sin() * self.head_radius;

            let point = tip - normalized_direction * self.head_length + (tan * x + bitan * y);

            vertices.push(GizmoVertex::new(point, self.color));
        }

        // mantel
        indices.extend(
            (0..self.segments)
                .flat_map(|i| [0, i + 1, (i + 1) % self.segments + 1])
                .collect::<Vec<_>>(),
        );

        // bottom
        indices.extend(
            (0..self.segments - 1)
                .flat_map(|i| [(i + 2) % self.segments + 1, i + 2, 1])
                .collect::<Vec<_>>(),
        );

        let start_index = vertices.len() as u32;

        // cylinder tail
        for offset in [0.0, 1.0] {
            for theta in 0..self.segments {
                let theta = theta as f32 * TAU / self.segments as f32 + DEG_45;

                let x = theta.cos() * self.tail_radius;
                let y = theta.sin() * self.tail_radius;

                let point = self.position
                    + normalized_direction * (offset * (length - self.head_length))
                    + (tan * x + bitan * y);

                vertices.push(GizmoVertex::new(point, self.color));
            }
        }

        // sides
        indices.extend(
            (0..self.segments)
                .flat_map(|i| {
                    [
                        start_index + i,
                        start_index + (i + 1) % self.segments,
                        start_index + i + self.segments,
                        start_index + (i + 1) % self.segments,
                        start_index + (i + 1) % self.segments + self.segments,
                        start_index + i + self.segments,
                    ]
                })
                .collect::<Vec<_>>(),
        );

        // bottom
        indices.extend(
            (0..self.segments - 1)
                .flat_map(|i| {
                    [
                        start_index + (i + 2) % self.segments,
                        start_index + i + 1,
                        start_index,
                    ]
                })
                .collect::<Vec<_>>(),
        );

        gizmos.add_geometry(&vertices, &indices);
    }
}
