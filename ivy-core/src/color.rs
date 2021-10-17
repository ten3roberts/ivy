use derive_more::*;
use ultraviolet::Vec4;

/// Color/tint of object
#[derive(
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
)]
#[repr(transparent)]
pub struct Color(pub Vec4);

impl Color {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self(Vec4::new(r, g, b, a))
    }

    pub fn red() -> Self {
        Self(Vec4::new(1.0, 0.0, 0.0, 1.0))
    }

    pub fn green() -> Self {
        Self(Vec4::new(0.0, 1.0, 0.0, 1.0))
    }

    pub fn blue() -> Self {
        Self(Vec4::new(0.0, 0.0, 1.0, 1.0))
    }

    pub fn cyan() -> Self {
        Self(Vec4::new(0.0, 1.0, 1.0, 1.0))
    }

    pub fn magenta() -> Self {
        Self(Vec4::new(1.0, 0.0, 1.0, 1.0))
    }
}
