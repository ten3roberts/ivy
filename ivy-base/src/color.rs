use derive_more::*;
use glam::{Vec3, Vec4};
use palette::{FromColor, Hsla, Hsva, Srgba};

#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};

/// Color/tint of an object
#[derive(
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
    PartialEq,
)]
#[repr(transparent)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct Color(pub Srgba);

impl Color {
    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self(Srgba::new(r, g, b, a))
    }

    pub fn rgb(r: f32, g: f32, b: f32) -> Self {
        Self(Srgba::new(r, g, b, 1.0))
    }

    pub fn hsla(h: f32, s: f32, l: f32, a: f32) -> Self {
        Self(Srgba::from_color(Hsla::new(h, s, l, a)))
    }

    pub fn hsl(h: f32, s: f32, l: f32) -> Self {
        Self(Srgba::from_color(Hsla::new(h, s, l, 1.0)))
    }

    pub fn hsva(h: f32, s: f32, v: f32, a: f32) -> Self {
        Self(Srgba::from_color(Hsva::new(h, s, v, a)))
    }

    pub fn hsv(h: f32, s: f32, v: f32) -> Self {
        Self(Srgba::from_color(Hsva::new(h, s, v, 1.0)))
    }
    pub fn white() -> Self {
        Self::rgba(1.0, 1.0, 1.0, 1.0)
    }

    pub fn black() -> Self {
        Self::rgba(0.0, 0.0, 0.0, 1.0)
    }

    pub fn gray() -> Self {
        Self::rgba(0.5, 0.5, 0.5, 1.0)
    }

    pub fn red() -> Self {
        Self::rgba(1.0, 0.0, 0.0, 1.0)
    }

    pub fn dark_red() -> Self {
        Self::rgba(0.5, 0.0, 0.0, 1.0)
    }
    pub fn green() -> Self {
        Self::rgba(0.0, 1.0, 0.0, 1.0)
    }

    pub fn dark_green() -> Self {
        Self::rgba(0.0, 0.0, 0.5, 1.0)
    }

    pub fn blue() -> Self {
        Self::rgba(0.0, 0.0, 1.0, 1.0)
    }

    pub fn dark_blue() -> Self {
        Self::rgba(0.0, 0.5, 0.0, 1.0)
    }

    pub fn cyan() -> Self {
        Self::rgba(0.0, 1.0, 1.0, 1.0)
    }

    pub fn magenta() -> Self {
        Self::rgba(1.0, 0.0, 1.0, 1.0)
    }

    pub fn purple() -> Self {
        Self::rgba(0.5, 0.0, 0.5, 1.0)
    }

    pub fn orange() -> Self {
        Self::rgba(1.0, 0.7, 0.0, 1.0)
    }

    pub fn yellow() -> Self {
        Self::rgba(1.0, 1.0, 0.0, 1.0)
    }
}

impl From<Vec3> for Color {
    fn from(v: Vec3) -> Self {
        Self(Srgba::new(v.x, v.y, v.z, 1.0))
    }
}

impl From<Color> for Vec4 {
    fn from(c: Color) -> Self {
        Vec4::new(c.red, c.green, c.blue, c.alpha)
    }
}

impl From<Color> for Vec3 {
    fn from(c: Color) -> Self {
        Vec3::new(c.red, c.green, c.blue)
    }
}

impl From<Vec4> for Color {
    fn from(v: Vec4) -> Self {
        Self(Srgba::new(v.x, v.y, v.z, v.w))
    }
}

impl From<&Vec3> for Color {
    fn from(v: &Vec3) -> Self {
        Self(Srgba::new(v.x, v.y, v.z, 1.0))
    }
}

impl From<&Color> for Vec4 {
    fn from(c: &Color) -> Self {
        Vec4::new(c.red, c.green, c.blue, c.alpha)
    }
}

impl From<&Color> for Vec3 {
    fn from(c: &Color) -> Self {
        Vec3::new(c.red, c.green, c.blue)
    }
}

impl From<&Vec4> for Color {
    fn from(v: &Vec4) -> Self {
        Self(Srgba::new(v.x, v.y, v.z, v.w))
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::white()
    }
}
