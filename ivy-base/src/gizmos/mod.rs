use std::collections::BTreeMap;

use glam::Vec3;

use crate::Color;

mod traits;
pub use traits::*;

/// A default radius that looks good for small gizmos
pub const DEFAULT_RADIUS: f32 = 0.02;

#[records::record]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Sphere {
    origin: Vec3,
    radius: f32,
}

impl Default for Sphere {
    fn default() -> Self {
        Self {
            origin: Default::default(),
            radius: DEFAULT_RADIUS,
        }
    }
}

impl DrawGizmos for Sphere {
    fn draw_gizmos(&self, gizmos: &mut Gizmos, color: Color) {
        gizmos.push(GizmoPrimitive::Sphere {
            origin: self.origin,
            color,
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
}

impl Line {
    pub fn from_points(a: Vec3, b: Vec3, radius: f32, corner_radius: f32) -> Self {
        Self {
            origin: a,
            dir: (b - a),
            radius,
            corner_radius,
        }
    }
}

impl DrawGizmos for Line {
    fn draw_gizmos(&self, gizmos: &mut Gizmos, color: Color) {
        gizmos.push(GizmoPrimitive::Line {
            origin: self.origin,
            color,
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
        }
    }
}

#[records::record]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Cube {
    origin: Vec3,
    half_extents: Vec3,
    radius: f32,
    corner_radius: f32,
}

impl Default for Cube {
    fn default() -> Self {
        Self {
            origin: Default::default(),
            radius: DEFAULT_RADIUS,
            half_extents: Vec3::ONE,
            corner_radius: 1.0,
        }
    }
}

impl DrawGizmos for Cube {
    fn draw_gizmos(&self, gizmos: &mut Gizmos, color: Color) {
        let sides = [
            ((Vec3::X, Vec3::Y)),
            ((Vec3::X, -Vec3::Y)),
            ((-Vec3::X, -Vec3::Y)),
            ((-Vec3::X, Vec3::Y)),
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

        let lines = sides.iter().map(|side| {
            let mid = self.origin + (side.0 + side.1) * self.half_extents;
            let dir = side.0.cross(side.1).normalize() * self.half_extents * 2.0;
            let pos = mid - dir * 0.5;

            GizmoPrimitive::Line {
                origin: pos,
                dir,
                corner_radius: self.corner_radius,
                color,
                radius: self.radius,
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
#[derive(Copy, Clone, PartialEq)]
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
    current: Option<Section>,
    sections: BTreeMap<Section, Vec<GizmoPrimitive>>,
}

impl Gizmos {
    pub fn new() -> Self {
        Self {
            current: None,
            sections: BTreeMap::new(),
        }
    }

    /// Begins a new section.
    /// If a section already exists with the same name, the existing gizmos will be
    /// cleared. If drawing singleton like types, consider using the typename as a
    /// section name.
    pub fn begin_section(&mut self, section: Section) {
        self.current = Some(section);
        self.sections.get_mut(section).map(Vec::clear);
    }

    pub fn clear_section(&mut self, section: Section) {
        self.sections.get_mut(section).map(Vec::clear);
    }

    /// Adds a new gizmos to the current section
    pub fn draw(&mut self, gizmo: impl DrawGizmos, color: Color) {
        gizmo.draw_gizmos(self, color)
    }

    pub fn push(&mut self, primitive: GizmoPrimitive) {
        self.get_section().push(primitive)
    }

    pub fn extend<I: Iterator<Item = GizmoPrimitive>>(&mut self, iter: I) {
        let section = self.get_section();

        section.extend(iter);
    }

    fn get_section(&mut self) -> &mut Vec<GizmoPrimitive> {
        self.sections
            .entry(
                self.current
                    .expect("Can not draw gizmos before initiating a section."),
            )
            .or_insert_with(Vec::new)
    }

    /// Get a reference to the gizmos's sections.
    pub fn sections(&self) -> &BTreeMap<Section, Vec<GizmoPrimitive>> {
        &self.sections
    }
}
