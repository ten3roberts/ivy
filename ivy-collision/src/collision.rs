use std::ops::Index;

use glam::Vec3;
use ivy_core::{Color, ColorExt, DrawGizmos, Line, Sphere};

use crate::{
    contact::{generate_contact_surface, ContactSurface},
    epa, gjk,
    util::minkowski_diff,
    EntityPayload, Shape,
};

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ContactPoints {
    Single([Vec3; 1]),
    Double([Vec3; 2]),
}

impl ContactPoints {
    pub fn single(p: Vec3) -> Self {
        Self::Single([p])
    }

    pub fn double(a: Vec3, b: Vec3) -> Self {
        Self::Double([a, b])
    }

    pub fn points(&self) -> &[Vec3] {
        match self {
            ContactPoints::Single(val) => val,
            ContactPoints::Double(val) => val,
        }
    }

    pub fn iter(&self) -> std::slice::Iter<Vec3> {
        self.into_iter()
    }

    pub fn reverse(&self) -> Self {
        match *self {
            Self::Single(p) => Self::Single(p),
            Self::Double([a, b]) => Self::Double([b, a]),
        }
    }
}

impl DrawGizmos for ContactPoints {
    fn draw_primitives(&self, gizmos: &mut ivy_core::GizmosSection) {
        for &p in self.iter() {
            gizmos.draw(Sphere {
                origin: p,
                color: Color::green(),
                ..Default::default()
            })
        }
    }
}

impl From<Vec3> for ContactPoints {
    fn from(val: Vec3) -> Self {
        Self::Single([val])
    }
}

impl From<[Vec3; 1]> for ContactPoints {
    fn from(val: [Vec3; 1]) -> Self {
        Self::Single(val)
    }
}

impl From<[Vec3; 2]> for ContactPoints {
    fn from(val: [Vec3; 2]) -> Self {
        Self::Double(val)
    }
}

impl<'a> IntoIterator for &'a ContactPoints {
    type Item = &'a Vec3;

    type IntoIter = std::slice::Iter<'a, Vec3>;

    fn into_iter(self) -> Self::IntoIter {
        match self {
            ContactPoints::Single(val) => val.iter(),
            ContactPoints::Double(val) => val.iter(),
        }
    }
}

impl Index<usize> for ContactPoints {
    type Output = Vec3;

    fn index(&self, index: usize) -> &Self::Output {
        &self.points()[index]
    }
}

#[derive(Debug, Clone)]
pub struct Penetration {
    /// The closest points on the two colliders, respectively
    pub points: ContactPoints,
    pub depth: f32,
    pub normal: Vec3,
    pub polytype: epa::Polytype,
}

#[derive(Debug, Clone)]
pub struct ContactManifold {
    pub surface: ContactSurface,
    pub penetration: Penetration,
}

impl DrawGizmos for Penetration {
    fn draw_primitives(&self, gizmos: &mut ivy_core::GizmosSection) {
        // gizmos.draw(self.points);

        gizmos.draw(Line {
            origin: self.points[0],
            dir: self.normal * 0.2,
            color: Color::blue(),
            ..Default::default()
        });

        self.polytype.draw_primitives(gizmos);
    }
}

impl DrawGizmos for ContactManifold {
    fn draw_primitives(&self, gizmos: &mut ivy_core::GizmosSection) {
        gizmos.draw(&self.surface);
        // gizmos.draw(&self.penetration);
    }
}

/// Represents a collision between two entities.
#[derive(Debug, Clone)]
pub struct Intersection {
    pub a: EntityPayload,
    pub b: EntityPayload,
    pub contact: ContactManifold,
}

pub fn intersect<A: Shape, B: Shape>(a: &A, b: &B) -> Option<ContactManifold> {
    let (intersect, simplex) = gjk(a, b);

    if !intersect {
        return None;
    }

    let contact_info = epa(simplex, |dir| minkowski_diff(a, b, dir));

    let surface =
        generate_contact_surface(a, b, contact_info.normal, contact_info.points.points()[0]);

    Some(ContactManifold {
        surface,
        penetration: contact_info,
    })
}
