use std::collections::HashMap;

use ultraviolet::Vec3;

use crate::{Color, Position};

mod traits;
pub use traits::*;

/// A default radius that looks good for small gizmos
pub const DEFAULT_RADIUS: f32 = 0.1;

#[derive(Copy, Clone, PartialEq)]
/// Represents a 3D world overlay for debugging purposes.
pub enum Gizmo {
    Sphere {
        origin: Position,
        color: Color,
        radius: f32,
    },
    Line {
        origin: Position,
        color: Color,
        dir: Vec3,
        radius: f32,
        // The radius of the corner, 0 is a straight corner, and 1 is a half
        // circle cap.
        corner_radius: f32,
    },
    Cube {
        origin: Position,
        color: Color,
        half_extents: Vec3,
        radius: f32,
    },
    Triangle {
        color: Color,
        points: [Vec3; 3],
        radius: f32,
    }, // /// The position of the gizmo
       // pos: Vec3,
       // /// The direction the gizmo is facing
       // dir: Vec3,
       // /// The half width of the gizmo
       // radius: f32,
       // /// THe gizmo color with transparency
       // color: Color,
       // /// The type of gizmos, controls how it is rendered.
       // kind: GizmoKind
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
    sections: HashMap<Section, Vec<Gizmo>>,
}

impl Gizmos {
    pub fn new() -> Self {
        Self {
            current: None,
            sections: HashMap::new(),
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

    /// Adds a new gizmos to the current section
    pub fn draw(&mut self, gizmo: Gizmo) {
        let section = self.get_section();

        section.push(gizmo);
    }

    pub fn extend<I: Iterator<Item = Gizmo>>(&mut self, iter: I) {
        let section = self.get_section();

        section.extend(iter);
    }

    fn get_section(&mut self) -> &mut Vec<Gizmo> {
        self.sections
            .entry(
                self.current
                    .expect("Can not draw gizmos before initiating a section."),
            )
            .or_insert_with(Vec::new)
    }

    /// Get a reference to the gizmos's sections.
    pub fn sections(&self) -> &HashMap<Section, Vec<Gizmo>> {
        &self.sections
    }
}
