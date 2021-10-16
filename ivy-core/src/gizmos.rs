use derive_more::*;
use ultraviolet::{Vec3, Vec4};

#[derive(Copy, Clone, PartialEq)]
pub struct Gizmo {
    /// The position of the gizmo
    pos: Vec3,
    /// The direction the gizmo is facing
    dir: Vec3,
    /// The half width of the gizmo
    radius: f32,
    /// THe gizmo color with transparency
    color: Vec4,
    /// The type of gizmos, controls how it is rendered.
    kind: GizmoKind,
}

impl Gizmo {
    pub fn new(pos: Vec3, dir: Vec3, radius: f32, color: Vec4, kind: GizmoKind) -> Self {
        Self {
            pos,
            dir,
            radius,
            color,
            kind,
        }
    }

    /// Get a reference to the gizmo's pos.
    #[inline]
    pub fn pos(&self) -> Vec3 {
        self.pos
    }

    /// Get a reference to the gizmo's color.
    #[inline]
    pub fn color(&self) -> Vec4 {
        self.color
    }

    /// Get a reference to the gizmo's kind.
    #[inline]
    pub fn kind(&self) -> &GizmoKind {
        &self.kind
    }

    pub fn billboard_axis(&self) -> Vec3 {
        match self.kind {
            GizmoKind::Sphere => Vec3::zero(),
            GizmoKind::Line => self.dir.normalized(),
        }
    }

    pub fn midpoint(&self) -> Vec3 {
        match self.kind {
            GizmoKind::Sphere => self.pos,
            GizmoKind::Line => self.pos + self.dir * 0.5,
        }
    }

    pub fn size(&self) -> Vec3 {
        match self.kind {
            GizmoKind::Sphere => Vec3::new(self.radius, self.radius, self.radius),
            GizmoKind::Line => self.dir * 0.5 + (Vec3::one() - self.dir.normalized()) * self.radius,
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum GizmoKind {
    Sphere,
    Line,
}

#[derive(Default, Deref, DerefMut)]
pub struct Gizmos(Vec<Gizmo>);

impl Gizmos {
    pub fn new() -> Self {
        Self(Vec::new())
    }
}
