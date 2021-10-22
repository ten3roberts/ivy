use std::collections::HashMap;

use ultraviolet::Vec3;

use crate::Color;

#[derive(Copy, Clone, PartialEq)]
/// Represents a 3D world overlay for debugging purposes.
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

pub type Section = &'static str;

/// Holds the gizmos to draw.
/// Before drawing gizmos, a section needs to be initiated. This will clear all
/// gizmos the section from previous calls and start adding subsequent gizmos to
/// the seciton. This is to separate clearing of gizmos drawn from layers and
/// systems of different intervals.
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

    pub fn begin_section(&mut self, section: Section) {
        self.current = Some(section);
        self.sections.get_mut(section).map(Vec::clear);
    }

    pub fn push(&mut self, gizmo: Gizmo) {
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
