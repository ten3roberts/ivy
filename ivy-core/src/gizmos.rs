use derive_more::*;
use ultraviolet::Vec3;

use crate::Color;

#[derive(Copy, Clone, PartialEq)]
pub enum Gizmo {
    Sphere {
        origin: Vec3,
        color: Color,
        radius: f32,
        // The radius of the corner, 0 is a straight corner, and 1 is a half
        // circle cap.
        corner_radius: f32,
    },
    Line {
        origin: Vec3,
        color: Color,
        dir: Vec3,
        radius: f32,
        // The radius of the corner, 0 is a straight corner, and 1 is a half
        // circle cap.
        corner_radius: f32,
    },
    Cube {
        origin: Vec3,
        color: Color,
        half_extents: Vec3,
        radius: f32,
        // The radius of the corner, 0 is a straight corner, and 1 is a half
        // circle cap.
        corner_radius: f32,
    },
    // /// The position of the gizmo
    // pos: Vec3,
    // /// The direction the gizmo is facing
    // dir: Vec3,
    // /// The half width of the gizmo
    // radius: f32,
    // /// THe gizmo color with transparency
    // color: Color,
    // /// The type of gizmos, controls how it is rendered.
    // kind: GizmoKind,
}

// impl Gizmo {
//     pub fn new(pos: Vec3, dir: Vec3, radius: f32, color: Color, kind: GizmoKind) -> Self {
//         Self {
//             pos,
//             dir,
//             radius,
//             color,
//             kind,
//         }
//     }

//     /// Get a reference to the gizmo's pos.
//     #[inline]
//     pub fn pos(&self) -> Vec3 {
//         self.pos
//     }

//     /// Get a reference to the gizmo's color.
//     #[inline]
//     pub fn color(&self) -> Color {
//         self.color
//     }

//     /// Get a reference to the gizmo's kind.
//     #[inline]
//     pub fn kind(&self) -> &GizmoKind {
//         &self.kind
//     }

//     pub fn billboard_axis(&self) -> Vec3 {
//         match self.kind {
//             GizmoKind::Sphere => Vec3::zero(),
//             _ => self.dir.normalized(),
//         }
//     }

//     pub fn midpoint(&self) -> Vec3 {
//         match self.kind {
//             GizmoKind::Line => self.pos + self.dir * 0.5,
//             _ => self.pos,
//         }
//     }

//     pub fn size(&self) -> Vec3 {
//         match self.kind {
//             GizmoKind::Line => self.dir * 0.5 + (Vec3::one() - self.dir.normalized()) * self.radius,
//             _ => Vec3::new(self.radius, self.radius, self.radius),
//         }
//     }
// }

// #[derive(Copy, Clone, PartialEq)]
// pub enum GizmoKind {
//     Sphere,
//     Line,
//     Cube,
// }

#[derive(Default, Deref, DerefMut)]
pub struct Gizmos(Vec<Gizmo>);

impl Gizmos {
    pub fn new() -> Self {
        Self(Vec::new())
    }
}
