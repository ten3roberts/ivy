use dashmap::DashMap;
use glam::{Mat4, Vec3};
use itertools::Itertools;

use crate::{Color, ColorExt};

mod traits;
pub use traits::*;

/// A default radius that looks good for small gizmos
pub const DEFAULT_RADIUS: f32 = 0.04;
pub const DEFAULT_THICKNESS: f32 = 0.02;

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Sphere {
    pub origin: Vec3,
    pub radius: f32,
    pub color: Color,
}

impl Sphere {
    pub fn new(origin: Vec3, radius: f32, color: Color) -> Self {
        Self {
            origin,
            radius,
            color,
        }
    }

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

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Line {
    pub origin: Vec3,
    pub dir: Vec3,
    pub radius: f32,
    pub color: Color,
}

impl Line {
    pub fn new(origin: Vec3, dir: Vec3, radius: f32, color: Color) -> Self {
        Self {
            origin,
            dir,
            radius,
            color,
        }
    }

    pub fn from_points(start: Vec3, end: Vec3, radius: f32, color: Color) -> Self {
        Self {
            origin: start,
            dir: (end - start),
            radius,
            color,
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
            corner_radius: 1.0,
        })
    }
}

impl Default for Line {
    fn default() -> Self {
        Self {
            origin: Default::default(),
            radius: DEFAULT_RADIUS,
            dir: Vec3::Z,
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
    pub transform: Mat4,
}

impl Cube {
    pub fn new(min: Vec3, max: Vec3, line_radius: f32, color: Color) -> Self {
        Self {
            min,
            max,
            line_radius,
            color,
            transform: Mat4::IDENTITY,
        }
    }

    /// Set the color
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }

    /// Set the transform
    pub fn with_transform(mut self, transform: Mat4) -> Self {
        self.transform = transform;
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
            transform: Mat4::IDENTITY,
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

        for side in sides.iter() {
            let mid = midpoint + (side.0 + side.1) * extent;
            let dir = side.0.cross(side.1).normalize() * (extent + self.line_radius) * 2.0;

            let start = mid - dir * 0.5;

            let end = start + dir;

            Line::from_points(
                self.transform.transform_point3(start),
                self.transform.transform_point3(end),
                self.line_radius,
                self.color,
            )
            .draw_primitives(gizmos);
            // GizmoPrimitive::Line {
            //     origin: pos,
            //     dir,
            //     corner_radius: 1.0,
            //     color: self.color,
            //     radius: self.line_radius,
            // }
        }
    }
}

pub struct Polygon<I> {
    pub points: I,
    pub color: Color,
}

impl<I> Polygon<I>
where
    I: IntoIterator<Item = Vec3>,
{
    pub fn new(points: I) -> Self {
        Self {
            points,
            color: Color::green(),
        }
    }

    /// Set the color
    pub fn with_color(mut self, color: Color) -> Self {
        self.color = color;
        self
    }
}

impl<I> DrawGizmos for Polygon<I>
where
    for<'x> I: Clone + IntoIterator<Item = Vec3>,
    for<'x> <I as IntoIterator>::IntoIter: Clone + ExactSizeIterator,
{
    fn draw_primitives(&self, gizmos: &mut GizmosSection) {
        for (p1, p2) in self.points.clone().into_iter().circular_tuple_windows() {
            gizmos.draw(Line::from_points(p1, p2, DEFAULT_THICKNESS, self.color));
            gizmos.draw(Sphere::new(p1, DEFAULT_RADIUS, self.color));
        }
    }
}

#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Triangle {
    origin: Vec3,
    points: [Vec3; 3],
    radius: f32,
    corner_radius: f32,
}

impl Triangle {
    pub fn new(origin: Vec3, points: [Vec3; 3], radius: f32, corner_radius: f32) -> Self {
        Self {
            origin,
            points,
            radius,
            corner_radius,
        }
    }
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
