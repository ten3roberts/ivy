use derive_more::{
    Add, AddAssign, AsRef, Deref, DerefMut, Div, DivAssign, From, Into, Mul, MulAssign, Sub,
    SubAssign,
};
use ultraviolet::{Mat4, Rotor3, Vec3};

#[derive(
    Add,
    AddAssign,
    AsRef,
    Clone,
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
pub struct Position(pub Vec3);

#[derive(
    Add,
    AddAssign,
    AsRef,
    Clone,
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
pub struct Rotation(pub Rotor3);

#[derive(
    Add,
    AddAssign,
    AsRef,
    Clone,
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
pub struct Scale(pub Vec3);

#[derive(
    Add,
    AddAssign,
    AsRef,
    Clone,
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

#[derive(
    Add,
    AddAssign,
    AsRef,
    Clone,
    Default,
    Deref,
    DerefMut,
    Div,
    DivAssign,
    From,
    Into,
    Mul,
    MulAssign,
)]
pub struct ModelMatrix(pub Mat4);
