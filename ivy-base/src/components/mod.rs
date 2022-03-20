use std::{borrow::Cow, time::Duration};

use derive_for::*;
use derive_more::*;
use glam::{Mat3, Mat4, Quat, Vec2, Vec3, Vec4Swizzles};
use hecs::{Bundle, DynamicBundleClone, Query};
use ivy_random::Random;
#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};
mod connections;
mod physics;
pub use connections::*;
pub use physics::*;

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
    Default,
    Neg,
    Display,
    );
    /// Describes a position in 3D space.
    #[repr(transparent)]
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub struct Position(pub Vec3);
    #[repr(transparent)]
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub struct Position2D(pub Vec2);
    #[repr(transparent)]
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub struct Size2D(pub Vec2);

    #[repr(transparent)]
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    /// Wrapper for strongly typed floating point deltatime
    pub struct DeltaTime(pub f32);
);

/// Describes a rotation in 3D space.
#[repr(transparent)]
#[derive(
    From, Into, Clone, Copy, DerefMut, Deref, Mul, MulAssign, PartialEq, Default, Debug, Display,
)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Rotation(pub Quat);
/// Describes a scale in 3D space.
/// Default is overridden for an identity scale.
#[repr(transparent)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
#[derive(
    From,
    Into,
    Clone,
    Copy,
    Deref,
    DerefMut,
    Div,
    Add,
    AddAssign,
    Sub,
    SubAssign,
    Mul,
    MulAssign,
    PartialEq,
    Debug,
    Display,
)]
pub struct Scale(pub Vec3);
impl From<Duration> for DeltaTime {
    fn from(val: Duration) -> Self {
        Self(val.as_secs_f32())
    }
}

impl std::ops::Mul<Position> for Rotation {
    type Output = Position;

    fn mul(self, rhs: Position) -> Self::Output {
        Position(self.0 * *rhs)
    }
}

impl std::ops::Mul<Rotation> for Rotation {
    type Output = Rotation;

    fn mul(self, rhs: Rotation) -> Self::Output {
        Rotation(self.0 * rhs.0)
    }
}

impl std::ops::MulAssign<Rotation> for Rotation {
    fn mul_assign(&mut self, rhs: Rotation) {
        *self = Rotation(self.0 * rhs.0)
    }
}

impl Default for Scale {
    fn default() -> Self {
        Self(Vec3::ONE)
    }
}

impl Position {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }

    pub fn zero() -> Self {
        Self(Vec3::new(0.0, 0.0, 0.0))
    }
}

impl Rotation {
    pub fn into_matrix3(&self) -> Mat3 {
        Mat3::from_quat(**self)
    }

    pub fn into_matrix(&self) -> Mat4 {
        Mat4::from_quat(**self)
    }

    /// Creates an euler rotation from yxz (yaw pitch roll) order
    pub fn euler_angles(angles: Vec3) -> Self {
        Self(Quat::from_euler(
            glam::EulerRot::YXZ,
            angles.y,
            angles.x,
            angles.z,
        ))
    }

    /// Creates an angular velocity from an axis angle rotation. Note: Axis is
    /// assumed to be normalized.
    pub fn axis_angle(axis: Vec3, angle: f32) -> Self {
        Self(Quat::from_axis_angle(axis, angle))
    }

    /// Creates a quaternion looking at `forward` with a roll facing `up`
    pub fn look_at(forward: Vec3, up: Vec3) -> Self {
        let forward = forward.reject_from(up).normalize_or_zero();
        if forward.is_nan() || forward.dot(Vec3::Z) > 0.99 {
            return Quat::IDENTITY.into();
        }

        let axis = Vec3::Z.cross(forward).normalize();

        assert!(forward.is_normalized());
        assert!(axis.is_normalized());

        let angle = forward.dot(Vec3::Z).acos();

        Self::axis_angle(axis, angle)
    }
}

impl Scale {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }

    pub fn uniform(val: f32) -> Self {
        Self(Vec3::new(val, val, val))
    }

    pub fn zero() -> Self {
        Self::uniform(0.0)
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
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct TransformMatrix(pub Mat4);

impl std::ops::Mul<TransformMatrix> for TransformMatrix {
    type Output = TransformMatrix;

    fn mul(self, rhs: TransformMatrix) -> Self::Output {
        TransformMatrix(self.0 * rhs.0)
    }
}

impl TransformMatrix {
    pub fn new(pos: Position, rot: Rotation, scale: Scale) -> Self {
        Self(Mat4::from_translation(*pos) * Mat4::from_quat(*rot) * Mat4::from_scale(*scale))
    }

    pub fn decompose(&self) -> (Scale, Rotation, Position) {
        let (scale, rot, pos) = self.to_scale_rotation_translation();
        (scale.into(), rot.into(), pos.into())
    }

    pub fn translation(&self) -> Position {
        self.col(3).xyz().into()
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
        **self.rot * Vec3::Z
    }

    /// Returns the right direction of the transform
    pub fn right(&self) -> Vec3 {
        **self.rot * Vec3::X
    }

    /// Returns the up direction of the transform
    pub fn up(&self) -> Vec3 {
        **self.rot * Vec3::Y
    }
}

#[derive(Default, Bundle, AsRef, PartialEq, Debug, Copy, Clone, DynamicBundleClone)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
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
/// Objects are assumed to remain in place and not move. Collisions between two
/// static objects will be ignored, useful for level objects which may overlap
/// but not generate collisions
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Static;

#[derive(Default, Debug, Clone, Copy)]
pub struct Sleeping;

/// Marker type for objects that will not interact with the physics system
/// through collisions despite having colliders.
#[derive(Default, Debug, Clone, Copy)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Trigger;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
/// Signifies if the entity should be visible or not. Default is true
pub enum Visible {
    /// Entity is fully visible
    Visible,
    /// Entity is explicitely hidden
    Hidden,
    /// Entity is hidden by a parent node
    HiddenInherit,
}

impl Visible {
    /// Returns true if it is considered visible
    #[inline]
    pub fn is_visible(self) -> bool {
        self == Self::Visible
    }

    /// Returns false if it is considered visible
    #[inline]
    pub fn is_hidden(self) -> bool {
        !self.is_visible()
    }
}

impl std::ops::Not for Visible {
    type Output = Self;

    fn not(self) -> Self::Output {
        if self.is_visible() {
            Self::Hidden
        } else {
            Self::Visible
        }
    }
}

impl Default for Visible {
    fn default() -> Self {
        Self::Visible
    }
}

#[derive(Clone, Eq, PartialEq, PartialOrd, Ord, Default, Hash, From, Into)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
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

macro_rules! impl_lerp {
    ([$(($ty: ident, $inner: ident),)*]) => {
        $(
            impl_lerp!($ty, $inner);
        )*
    };
    ($ty: ident, $inner: ident) => {
        impl<'a> ezy::Lerp<'a> for $ty {
            type Write = &'a mut Self;

            fn lerp(write: Self::Write, start: &Self, end: &Self, t: f32) {
                <$inner as ezy::Lerp>::lerp(&mut write.0, start, end, t)
            }
        }
    };
}

impl_lerp!([(Position, Vec3), (Scale, Vec3),]);
