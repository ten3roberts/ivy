use std::{
    iter::FromIterator,
    ops::{Deref, Index},
};

use hecs::Entity;
use smallvec::SmallVec;
use ultraviolet::{Mat4, Vec3};

use crate::{epa, gjk, util::minkowski_diff, CollisionPrimitive};

#[derive(Debug, Clone, PartialEq)]
pub struct ContactPoints(SmallVec<[Vec3; 4]>);

impl ContactPoints {
    pub fn new(points: &[Vec3]) -> Self {
        Self(SmallVec::from_slice(points))
    }

    pub fn from_iter<I: Iterator<Item = Vec3>>(iter: I) -> Self {
        Self(SmallVec::from_iter(iter))
    }

    pub fn points(&self) -> &[Vec3] {
        &self.0
    }

    pub fn iter<'a>(&'a self) -> std::slice::Iter<'a, Vec3> {
        self.into_iter()
    }
}

impl From<Vec<Vec3>> for ContactPoints {
    fn from(val: Vec<Vec3>) -> Self {
        Self(SmallVec::from_vec(val))
    }
}

impl<T: Deref<Target = Vec3>> From<&[T]> for ContactPoints {
    fn from(val: &[T]) -> Self {
        Self(SmallVec::from_iter(val.into_iter().map(|val| **val)))
    }
}

impl<'a> IntoIterator for &'a ContactPoints {
    type Item = &'a Vec3;

    type IntoIter = std::slice::Iter<'a, Vec3>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
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
                    &a_transform,
                    &b_transform,
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
