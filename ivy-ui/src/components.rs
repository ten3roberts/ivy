use derive_more::*;
use ultraviolet::{Mat4, Vec2};

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
pub struct Position2D(pub Vec2);

impl Position2D {
    pub fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }
}

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
pub struct Size2D(pub Vec2);

impl Size2D {
    pub fn new(x: f32, y: f32) -> Self {
        Self(Vec2::new(x, y))
    }
}

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
)]
pub struct ModelMatrix(pub Mat4);

/// Marker type for UI and the UI hierarchy.
pub struct Widget;
