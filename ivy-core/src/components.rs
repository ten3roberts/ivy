use derive_for::*;
use derive_more::*;
use ultraviolet::{Bivec3, Mat4, Rotor3, Vec3};

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
    Default,
    PartialEq,
    );
    /// Describes a position in 3D space.
    #[repr(transparent)]
    pub struct Position(pub Vec3);
    /// Describes a rotation in 3D space.
    #[repr(transparent)]
    pub struct Rotation(pub Rotor3);
    /// Describes a scale in 3D space.
    #[repr(transparent)]
    pub struct Scale(pub Vec3);
);

impl Position {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
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
        Self(Rotor3::new(angle, Bivec3::from_normalized_axis(axis)))
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

#[derive(
    AsRef, PartialEq, Clone, Copy, Debug, Default, Deref, DerefMut, From, Into, Mul, MulAssign,
)]
#[repr(transparent)]
/// A matrix transforming a point from local space to world space. This can
/// be used to transform a direction relative to the entity to be relative to
/// the world.
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
