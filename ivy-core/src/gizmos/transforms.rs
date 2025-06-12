use std::f32::consts::TAU;

use glam::{vec3, Mat4, Quat, Vec3};
use palette::Srgba;

use crate::{math::Axis3D, Color, ColorExt, DEG_45};

use super::{DrawGizmos, GizmoVertex, GizmosSection};

pub struct RotateGizmo {
    pub transform: Mat4,
    pub radius: f32,
    pub tube_radius: f32,
    pub colors: [Srgba; 3],
}

impl RotateGizmo {
    pub fn new(transform: Mat4, colors: [Srgba; 3]) -> Self {
        Self {
            transform,
            radius: 1.0,
            colors,
            tube_radius: 0.02,
        }
    }

    pub fn with_radius(mut self, size: f32) -> Self {
        self.radius = size;
        self
    }

    pub fn with_tube_radius(mut self, tube_radius: f32) -> Self {
        self.tube_radius = tube_radius;
        self
    }
}

impl DrawGizmos for RotateGizmo {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        let axes = [
            (Axis3D::X, self.colors[0]),
            (Axis3D::Y, self.colors[1]),
            (Axis3D::Z, self.colors[2]),
        ];

        let position = self.transform.transform_point3(Vec3::ZERO);

        for (axis, color) in axes.iter() {
            let direction = self.transform.transform_vector3(axis.to_vec3()).normalize();

            gizmos.draw(
                ToroidGizmo::new(position, direction)
                    .with_color(*color)
                    .with_radius(self.radius)
                    .with_tube_radius(self.tube_radius)
                    .with_segments(8)
                    .with_sections(48),
            )
        }
    }
}

pub struct TranslateGizmo {
    pub transform: Mat4,
    pub size: f32,
    pub colors: [Srgba; 3],
}

impl TranslateGizmo {
    pub fn new(transform: Mat4, colors: [Srgba; 3]) -> Self {
        Self {
            transform,
            colors,
            size: 1.0,
        }
    }

    pub fn with_size(mut self, size: f32) -> Self {
        self.size = size;
        self
    }
}

impl DrawGizmos for TranslateGizmo {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        let axes = [
            (Axis3D::X, self.colors[0]),
            (Axis3D::Y, self.colors[1]),
            (Axis3D::Z, self.colors[2]),
        ];

        let position = self.transform.transform_point3(Vec3::ZERO);

        for (axis, color) in axes.iter() {
            let direction = self.transform.transform_vector3(axis.to_vec3()).normalize();

            gizmos.draw(
                ArrowGizmo::new(position, direction)
                    .with_color(*color)
                    .with_scale(self.size),
            );
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
    scale: f32,
    color: Srgba,
}

impl ArrowGizmo {
    pub fn new(position: Vec3, direction: Vec3) -> Self {
        Self {
            head_length: 0.15,
            head_radius: 0.06,
            tail_radius: 0.015,
            segments: 12,
            direction,
            color: Srgba::new(1.0, 0.0, 0.0, 1.0),
            position,
            scale: 1.0,
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

    pub fn with_scale(mut self, scale: f32) -> Self {
        self.scale = scale;
        self
    }
}

impl DrawGizmos for ArrowGizmo {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        let mut writer = gizmos.add_geometry();

        let tan = if self.direction.distance(Vec3::Y) < 0.001 {
            Vec3::Z
        } else {
            self.direction.cross(Vec3::Y).normalize()
        };

        let length = self.direction.length();
        let normalized_direction = self.direction.normalize();

        let tip = self.position + (normalized_direction * length) * self.scale;
        writer.add_vertex(GizmoVertex::new(tip, self.color));

        let bitan = normalized_direction.cross(tan);

        for theta in 0..self.segments {
            let theta = theta as f32 * TAU / self.segments as f32 + DEG_45;

            let x = theta.cos() * self.head_radius;
            let y = theta.sin() * self.head_radius;

            let point = tip
                - (normalized_direction * self.head_length + (tan * x + bitan * y)) * self.scale;

            writer.add_vertex(GizmoVertex::new(point, self.color));
        }

        // mantel
        writer
            .add_indices((0..self.segments).flat_map(|i| [0, i + 1, (i + 1) % self.segments + 1]));

        // bottom
        writer.add_indices(
            (0..self.segments - 1).flat_map(|i| [(i + 2) % self.segments + 1, i + 2, 1]),
        );

        let start_index = self.segments + 1;

        // cylinder tail
        for offset in [0.0, 1.0] {
            for theta in 0..self.segments {
                let theta = theta as f32 * TAU / self.segments as f32 + DEG_45;

                let x = theta.cos() * self.tail_radius;
                let y = theta.sin() * self.tail_radius;

                let point = self.position
                    + (normalized_direction * (offset * (length - self.head_length))
                        + (tan * x + bitan * y))
                        * self.scale;

                writer.add_vertex(GizmoVertex::new(point, self.color));
            }
        }

        // sides
        writer.add_indices((0..self.segments).flat_map(|i| {
            [
                start_index + i,
                start_index + (i + 1) % self.segments,
                start_index + i + self.segments,
                start_index + (i + 1) % self.segments,
                start_index + (i + 1) % self.segments + self.segments,
                start_index + i + self.segments,
            ]
        }));

        // bottom
        writer.add_indices((0..self.segments - 1).flat_map(|i| {
            [
                start_index + (i + 2) % self.segments,
                start_index + i + 1,
                start_index,
            ]
        }));
    }
}

pub struct ToroidGizmo {
    radius: f32,
    tube_radius: f32,
    segments: u32,
    sections: u32,
    color: Srgba,
    position: Vec3,
    up: Vec3,
}

impl ToroidGizmo {
    pub fn new(position: Vec3, up: Vec3) -> Self {
        Self {
            radius: 1.0,
            tube_radius: 0.2,
            segments: 8,
            sections: 32,
            color: Color::white(),
            position,
            up,
        }
    }

    pub fn with_color(mut self, color: Srgba) -> Self {
        self.color = color;
        self
    }

    pub fn with_radius(mut self, radius: f32) -> Self {
        self.radius = radius;
        self
    }

    pub fn with_tube_radius(mut self, tube_radius: f32) -> Self {
        self.tube_radius = tube_radius;
        self
    }

    pub fn with_segments(mut self, segments: u32) -> Self {
        self.segments = segments;
        self
    }

    pub fn with_sections(mut self, sections: u32) -> Self {
        self.sections = sections;
        self
    }
}

impl DrawGizmos for ToroidGizmo {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        let mut writer = gizmos.add_geometry();

        let rotation = Quat::from_rotation_arc(Vec3::Y, self.up).normalize();

        for phi in 0..self.sections {
            let phi = phi as f32 * TAU / self.sections as f32;

            for theta in 0..self.segments {
                let theta = theta as f32 * TAU / self.segments as f32;

                let x = (self.radius + self.tube_radius * theta.cos()) * phi.cos();
                let y = self.tube_radius * theta.sin();
                let z = (self.radius + self.tube_radius * theta.cos()) * phi.sin();

                let point = self.position + rotation * vec3(x, y, z);

                writer.add_vertex(GizmoVertex::new(point, self.color));
            }
        }

        for phi in 0..self.sections {
            for theta in 0..self.segments {
                let next_phi = (phi + 1) % self.sections;
                let next_theta = (theta + 1) % self.segments;

                writer.add_indices([
                    (phi * self.segments + next_theta),
                    (next_phi * self.segments + theta),
                    (phi * self.segments + theta),
                    (phi * self.segments + next_theta),
                    (next_phi * self.segments + next_theta),
                    (next_phi * self.segments + theta),
                ]);
            }
        }
    }
}
