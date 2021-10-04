use derive_for::*;
use derive_more::*;
use ultraviolet::{Rotor3, Vec3};

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
    );
    /// Describes a position in 3D space.
pub struct Position(pub Vec3);
    /// Describes a rotation in 3D space.
pub struct Rotation(pub Rotor3);
/// Describes a scale in 3D space.
pub struct Scale(pub Vec3);
);

impl Position {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }
}

impl Rotation {
    pub fn new(roll: f32, pitch: f32, yaw: f32) -> Self {
        Self(Rotor3::from_euler_angles(roll, pitch, yaw))
    }
}

impl Scale {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }
}
