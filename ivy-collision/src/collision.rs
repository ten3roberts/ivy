use std::ops::Index;

use hecs::Entity;
use ultraviolet::{Mat4, Vec3};

use crate::{epa, gjk, util::minkowski_diff, CollisionPrimitive};

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

#[derive(Debug, Clone, PartialEq)]
pub struct Contact {
    /// The closest points on the two colliders, respectively
    pub points: ContactPoints,
    pub depth: f32,
    pub normal: Vec3,
}

/// Represents a collision between two entities.
#[derive(Debug, Clone)]
pub struct Collision {
    pub a: Entity,
    pub b: Entity,
    pub contact: Contact,
}

pub fn intersect<A: CollisionPrimitive, B: CollisionPrimitive>(
    a_transform: &Mat4,
    b_transform: &Mat4,
    a: &A,
    b: &B,
) -> Option<Contact> {
    let a_transform_inv = a_transform.inversed();
    let b_transform_inv = b_transform.inversed();

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
