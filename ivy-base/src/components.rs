use std::borrow::Cow;

use derive_for::*;
use derive_more::*;
use hecs::{Bundle, Query};
use ivy_random::Random;
use ultraviolet::{Bivec3, Mat4, Rotor3, Vec2, Vec3};

derive_for!(

    (
    Add,
    AddAssign,
    AsRef,
    Clone,
    Copy,
    Debug,
    Deref,
    DerefMut,
    Div,
    DivAssign,
    From,
    Into,
    Mul,
    MulAssign,
    Sub,
    SubAssign,
    PartialEq,
    );
    /// Describes a position in 3D space.
    #[repr(transparent)]
    #[derive(Default, Neg)]
    pub struct Position(pub Vec3);
    /// Describes a rotation in 3D space.
    #[repr(transparent)]
    #[derive(Default)]
    pub struct Rotation(pub Rotor3);
    /// Describes a scale in 3D space.
    /// Default is overridden for an identity scale.
    #[repr(transparent)]
    pub struct Scale(pub Vec3);
    #[repr(transparent)]
    #[derive(Default, Neg)]
    pub struct Position2D(pub Vec2);
    #[repr(transparent)]
    #[derive(Default)]
    pub struct Size2D(pub Vec2);
);

impl std::ops::Mul<Position> for Rotation {
    type Output = Position;

    fn mul(self, rhs: Position) -> Self::Output {
        Position(self.0 * *rhs)
    }
}

impl Default for Scale {
    fn default() -> Self {
        Self(Vec3::one())
    }
}

impl Position {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }
}

impl Rotation {
    pub fn euler_angles(roll: f32, pitch: f32, yaw: f32) -> Self {
        Self(Rotor3::from_euler_angles(roll, pitch, yaw))
    }
    /// Creates an angular velocity from an axis angle rotation. Note: Axis is
    /// assumed to be normalized.
    pub fn axis_angle(angle: f32, axis: Vec3) -> Self {
        Self(Rotor3::from_angle_plane(
            angle,
            Bivec3::from_normalized_axis(axis),
        ))
    }
}

impl Scale {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }

    pub fn uniform(val: f32) -> Self {
        Self(Vec3::new(val, val, val))
    }
}

impl Position2D {
    pub fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }
}

impl Size2D {
    pub fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }
}

#[derive(
    AsRef, PartialEq, Clone, Copy, Debug, Default, Deref, DerefMut, From, Into, Mul, MulAssign,
)]
#[repr(transparent)]
/// A matrix transforming a point from local space to world space. This can
/// be used ~                                               │                                                                                              │to transform a direction relative to the entity to be relative to
/// the world.
/// Should not be inserted into the world as it can become outdated when either
/// Position, Rotation, or Scale changes. Use TransformQuery instead.
pub struct TransformMatrix(pub Mat4);

impl TransformMatrix {
    pub fn new(pos: Position, rot: Rotation, scale: Scale) -> Self {
        Self(
            Mat4::from_translation(*pos)
                * rot.into_matrix().into_homogeneous()
                * Mat4::from_nonuniform_scale(*scale),
        )
    }
}

impl<'a> From<TransformQuery<'a>> for TransformMatrix {
    fn from(val: TransformQuery<'a>) -> Self {
        val.into_matrix()
    }
}

#[derive(Query, AsRef, PartialEq, Debug, Copy, Clone)]
/// Represents a query for Position, Rotation, and Scale.
/// Can easily be converted into a TransformMatrix.
pub struct TransformQuery<'a> {
    pub pos: &'a Position,
    pub rot: &'a Rotation,
    pub scale: &'a Scale,
}

impl<'a> TransformQuery<'a> {
    /// Converts the query into a transform matrix
    pub fn into_matrix(&self) -> TransformMatrix {
        TransformMatrix::new(*self.pos, *self.rot, *self.scale)
    }

    pub fn into_owned(&self) -> TransformBundle {
        TransformBundle {
            pos: *self.pos,
            rot: *self.rot,
            scale: *self.scale,
        }
    }

    /// Returns the forward direction of the transform
    pub fn forward(&self) -> Vec3 {
        self.rot.into_matrix() * Vec3::unit_z()
    }

    /// Returns the right direction of the transform
    pub fn right(&self) -> Vec3 {
        self.rot.into_matrix() * Vec3::unit_x()
    }

    /// Returns the up direction of the transform
    pub fn up(&self) -> Vec3 {
        self.rot.into_matrix() * Vec3::unit_y()
    }
}

#[derive(Default, Bundle, AsRef, PartialEq, Debug, Copy, Clone)]
pub struct TransformBundle {
    pub pos: Position,
    pub rot: Rotation,
    pub scale: Scale,
}

impl TransformBundle {
    pub fn new(pos: Position, rot: Rotation, scale: Scale) -> Self {
        Self { pos, rot, scale }
    }

    /// Converts the query into a transform matrix
    pub fn into_matrix(&self) -> TransformMatrix {
        TransformMatrix::new(self.pos, self.rot, self.scale)
    }
}

#[derive(Query, AsRef, PartialEq, Debug)]
/// Represents a query for Position, Rotation, and Scale.
/// Can easily be converted into a TransformMatrix.
pub struct TransformQueryMut<'a> {
    pub pos: &'a mut Position,
    pub rot: &'a mut Rotation,
    pub scale: &'a mut Scale,
}

impl<'a> TransformQueryMut<'a> {
    /// Converts the query into a transform matrix
    pub fn into_matrix(&self) -> TransformMatrix {
        TransformMatrix::new(*self.pos, *self.rot, *self.scale)
    }
}

// Impl random

impl Random for Position {
    fn rand_unit<R: ivy_random::rand::Rng>(rng: &mut R) -> Self {
        Position(Vec3::rand_unit(rng))
    }

    fn rand_sphere<R: ivy_random::rand::Rng>(rng: &mut R) -> Self {
        Position(Vec3::rand_sphere(rng))
    }

    fn rand_constrained_sphere<R: ivy_random::rand::Rng>(rng: &mut R, r1: f32, r2: f32) -> Self {
        Position(Vec3::rand_constrained_sphere(rng, r1, r2))
    }

    fn rand_uniform<R: ivy_random::rand::Rng>(rng: &mut R) -> Self {
        Position(Vec3::rand_uniform(rng))
    }
}

impl Random for Scale {
    fn rand_unit<R: ivy_random::rand::Rng>(rng: &mut R) -> Self {
        Scale(Vec3::rand_unit(rng))
    }

    fn rand_sphere<R: ivy_random::rand::Rng>(rng: &mut R) -> Self {
        Scale(Vec3::rand_sphere(rng))
    }

    fn rand_constrained_sphere<R: ivy_random::rand::Rng>(rng: &mut R, r1: f32, r2: f32) -> Self {
        Scale(Vec3::rand_constrained_sphere(rng, r1, r2))
    }

    fn rand_uniform<R: ivy_random::rand::Rng>(rng: &mut R) -> Self {
        Scale(Vec3::rand_uniform(rng))
    }
}

impl Random for Position2D {
    fn rand_unit<R: ivy_random::rand::Rng>(rng: &mut R) -> Self {
        Position2D(Vec2::rand_unit(rng))
    }

    fn rand_sphere<R: ivy_random::rand::Rng>(rng: &mut R) -> Self {
        Position2D(Vec2::rand_sphere(rng))
    }

    fn rand_constrained_sphere<R: ivy_random::rand::Rng>(rng: &mut R, r1: f32, r2: f32) -> Self {
        Position2D(Vec2::rand_constrained_sphere(rng, r1, r2))
    }

    fn rand_uniform<R: ivy_random::rand::Rng>(rng: &mut R) -> Self {
        Position2D(Vec2::rand_uniform(rng))
    }
}

impl Random for Size2D {
    fn rand_unit<R: ivy_random::rand::Rng>(rng: &mut R) -> Self {
        Size2D(Vec2::rand_unit(rng))
    }

    fn rand_sphere<R: ivy_random::rand::Rng>(rng: &mut R) -> Self {
        Size2D(Vec2::rand_sphere(rng))
    }

    fn rand_constrained_sphere<R: ivy_random::rand::Rng>(rng: &mut R, r1: f32, r2: f32) -> Self {
        Size2D(Vec2::rand_constrained_sphere(rng, r1, r2))
    }

    fn rand_uniform<R: ivy_random::rand::Rng>(rng: &mut R) -> Self {
        Size2D(Vec2::rand_uniform(rng))
    }
}

#[derive(Default, Debug, Clone, Copy)]
/// Marker type for objects that will not move through physics or other means.
pub struct Static;

/// Marker type for objects that will not interact with the physics system
/// through collisions despite having colliders.
#[derive(Default, Debug, Clone, Copy)]
pub struct Trigger;

#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Default, Hash, From, Into)]
pub struct Name(Cow<'static, str>);

impl Name {
    pub fn new<S: Into<Cow<'static, str>>>(name: S) -> Self {
        Self(name.into())
    }
}

impl std::ops::Deref for Name {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl std::ops::DerefMut for Name {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.0.to_mut()
    }
}
