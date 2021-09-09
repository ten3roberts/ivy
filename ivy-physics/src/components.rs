use derive_more::*;
use ultraviolet::Vec3;

#[derive(
    Add,
    AddAssign,
    AsRef,
    Clone,
    Copy,
    Debug,
    Default,
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
)]
pub struct Velocity(pub Vec3);

#[derive(
    Add,
    AddAssign,
    AsRef,
    Clone,
    Copy,
    Debug,
    Default,
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
)]
pub struct AngularVelocity(pub Vec3);
