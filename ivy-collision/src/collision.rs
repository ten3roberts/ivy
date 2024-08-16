use std::ops::Index;

use glam::{Mat4, Vec3};
use ivy_core::{Color, ColorExt, DrawGizmos, Line, Sphere, Triangle};

use crate::{epa, gjk, util::minkowski_diff, CollisionPrimitive, EntityPayload};

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
                radius: 0.1,
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
pub struct Contact {
    /// The closest points on the two colliders, respectively
    pub points: ContactPoints,
    pub depth: f32,
    pub normal: Vec3,
    pub polytype: epa::Polytype,
}

impl DrawGizmos for Contact {
    fn draw_primitives(&self, gizmos: &mut ivy_core::GizmosSection) {
        gizmos.draw(self.points);

        gizmos.draw(Line {
            origin: self.points[0],
            dir: self.normal * 0.2,
            color: Color::blue(),
            ..Default::default()
        });

        for face in &self.polytype.faces {
            for edge in face.edges() {
                let p1 = self.polytype.points[edge.0 as usize];
                let p2 = self.polytype.points[edge.1 as usize];

                gizmos.draw(Line::from_points(
                    p1.support,
                    p2.support,
                    0.01,
                    Color::cyan(),
                ))
            }
        }
    }
}

/// Represents a collision between two entities.
#[derive(Debug, Clone)]
pub struct Collision {
    pub a: EntityPayload,
    pub b: EntityPayload,
    pub contact: Contact,
}

pub fn intersect<A: CollisionPrimitive, B: CollisionPrimitive>(
    a_transform: &Mat4,
    b_transform: &Mat4,
    a: &A,
    b: &B,
) -> Option<Contact> {
    let a_transform_inv = a_transform.inverse();
    let b_transform_inv = b_transform.inverse();

    let (intersect, simplex) = gjk(
        a_transform,
        b_transform,
        &a_transform_inv,
        &b_transform_inv,
        a,
        b,
    );

    if intersect {
        Some(epa(
            |dir| {
                minkowski_diff(
                    a_transform,
                    b_transform,
                    &a_transform_inv,
                    &b_transform_inv,
                    a,
                    b,
                    dir,
                )
            },
            simplex,
        ))
    } else {
        None
    }
}
