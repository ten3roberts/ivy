use derive_more::{Add, AsRef, Deref, Div, From, Into, Mul, Sub};
use ultraviolet::{Mat4, Rotor3, Vec3};

#[derive(Default, Deref, Clone, Add, Mul, Div, Sub, From, Into, AsRef)]
pub struct Position(pub Vec3);

#[derive(Default, Deref, Clone, Add, Mul, Div, Sub, From, Into, AsRef)]
pub struct Rotation(pub Rotor3);

#[derive(Default, Deref, Clone, Add, Mul, Div, Sub, From, Into, AsRef)]
pub struct Scale(pub Vec3);

#[derive(Default, Deref, Clone, Add, Mul, Div, Sub, From, Into, AsRef)]
pub struct AngularVelocity(pub Vec3);

#[derive(Default, Deref, Clone, Add, Mul, Div, From, Into, AsRef)]
pub struct ModelMatrix(pub Mat4);
