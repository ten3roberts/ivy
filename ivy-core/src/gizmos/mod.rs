use dashmap::DashMap;
use glam::Vec3;

use crate::{Color, ColorExt};

mod traits;
pub use traits::*;

/// A default radius that looks good for small gizmos
pub const DEFAULT_RADIUS: f32 = 0.01;

#[records::record]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Sphere {
    origin: Vec3,
    radius: f32,
    color: Color,
}

impl Sphere {
    /// Set the color
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

impl Default for Sphere {
    fn default() -> Self {
        Self {
            origin: Default::default(),
            radius: DEFAULT_RADIUS,
            color: Color::red(),
        }
    }
}

impl DrawGizmos for Sphere {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        gizmos.push(GizmoPrimitive::Sphere {
            origin: self.origin,
            color: self.color,
            radius: self.radius,
        })
    }
}

#[records::record]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Line {
    origin: Vec3,
    dir: Vec3,
    radius: f32,
    corner_radius: f32,
    color: Color,
}

impl Line {
    pub fn from_points(a: Vec3, b: Vec3, radius: f32, corner_radius: f32) -> Self {
        Self {
            origin: a,
            dir: (b - a),
            radius,
            corner_radius,
            color: Color::blue(),
        }
    }

    /// Set the color
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

impl DrawGizmos for Line {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        gizmos.push(GizmoPrimitive::Line {
            origin: self.origin,
            color: self.color,
            dir: self.dir,
            radius: self.radius,
            corner_radius: self.corner_radius,
        })
    }
}

impl Default for Line {
    fn default() -> Self {
        Self {
            origin: Default::default(),
            radius: DEFAULT_RADIUS,
            dir: Vec3::Z,
            corner_radius: 1.0,
            color: Color::blue(),
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Cube {
    pub min: Vec3,
    pub max: Vec3,
    pub line_radius: f32,
    pub color: Color,
}

impl Cube {
    /// Set the color
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

impl Default for Cube {
    fn default() -> Self {
        Self {
            min: Vec3::ZERO,
            max: Vec3::ZERO,
            line_radius: 0.02,
            color: Color::green(),
        }
    }
}

impl DrawGizmos for Cube {
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        let sides = [
            (Vec3::X, Vec3::Y),
            (Vec3::X, -Vec3::Y),
            (-Vec3::X, -Vec3::Y),
            (-Vec3::X, Vec3::Y),
            // --
            (Vec3::Z, Vec3::Y),
            (Vec3::Z, -Vec3::Y),
            (-Vec3::Z, -Vec3::Y),
            (-Vec3::Z, Vec3::Y),
            // --
            (Vec3::X, Vec3::Z),
            (Vec3::X, -Vec3::Z),
            (-Vec3::X, -Vec3::Z),
            (-Vec3::X, Vec3::Z),
        ];

        let midpoint = (self.max + self.min) / 2.0;
        let extent = (self.max - self.min) / 2.0;

        let lines = sides.iter().map(|side| {
            let mid = midpoint + (side.0 + side.1) * extent;
            let dir = side.0.cross(side.1).normalize() * (extent + self.line_radius) * 2.0;

            let pos = mid - dir * 0.5;

            GizmoPrimitive::Line {
                origin: pos,
                dir,
                corner_radius: 1.0,
                color: self.color,
                radius: self.line_radius,
            }
        });

        gizmos.extend(lines)
    }
}

#[records::record]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Triangle {
    origin: Vec3,
    points: [Vec3; 3],
    radius: f32,
    corner_radius: f32,
}

impl Default for Triangle {
    fn default() -> Self {
        Self {
            origin: Default::default(),
            radius: DEFAULT_RADIUS,
            points: [Vec3::X, Vec3::Y, Vec3::Z],
            corner_radius: 1.0,
        }
    }
}
#[derive(Debug, Copy, Clone, PartialEq)]
/// Represents a 3D world overlay for debugging purposes.
pub enum GizmoPrimitive {
    Sphere {
        origin: Vec3,
        color: Color,
        radius: f32,
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
}

pub type Section = &'static str;

/// Holds the gizmos to draw.
/// Before drawing gizmos, a section needs to be initiated. This will clear all
/// gizmos the section from previous calls and start adding subsequent gizmos to
/// the seciton. This is to separate clearing of gizmos drawn from layers and
/// systems of different intervals.
///
/// The API works much like an immediate mode GUI, except different sections are
/// transient at different durations.
#[derive(Default)]
pub struct Gizmos {
    sections: DashMap<&'static str, GizmosSection>,
}

impl Gizmos {
    pub fn new() -> Self {
        Self {
            sections: Default::default(),
        }
    }

    /// Begins a new section.
    /// If a section already exists with the same name, the existing gizmos will be
    /// cleared. If drawing singleton like types, consider using the typename as a
    /// section name.
    pub fn begin_section<'a>(
        &'a self,
        key: &'static str,
    ) -> dashmap::mapref::one::RefMut<'a, &'static str, GizmosSection> {
        // if let Some(mut section) = self.sections.get_mut(key) {
        //     section.primitives.clear();
        //     section
        // } else {
        self.sections
            .entry(key)
            .and_modify(|v| v.primitives.clear())
            .or_default()
        // }
    }

    /// Get a reference to the gizmos's sections.
    pub fn sections(
        &self,
    ) -> dashmap::iter::Iter<
        '_,
        &'static str,
        GizmosSection,
        std::hash::RandomState,
        DashMap<&'static str, GizmosSection>,
    > {
        self.sections.iter()
    }
}

#[derive(Default, Debug, Clone)]
pub struct GizmosSection {
    primitives: Vec<GizmoPrimitive>,
}

impl GizmosSection {
    /// Adds a new gizmos to the current section
    pub fn draw(&mut self, gizmo: impl DrawGizmos) {
        gizmo.draw_primitives(self)
    }

    pub fn push(&mut self, primitive: GizmoPrimitive) {
        self.primitives.push(primitive)
    }

    pub fn extend<I: Iterator<Item = GizmoPrimitive>>(&mut self, iter: I) {
        self.primitives.extend(iter);
    }

    pub fn primitives(&self) -> &[GizmoPrimitive] {
        &self.primitives
    }
}
