use glam::{Vec3, Vec4};
use palette::{FromColor, Hsla, Hsva, Srgba};

#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};

pub type Color = Srgba;

pub trait ColorExt {
    fn to_vec3(&self) -> Vec3;
    fn to_vec4(&self) -> Vec4;
    fn to_hsva(&self) -> Hsva;
    fn to_hsla(&self) -> Hsla;
    fn red() -> Self;

    fn green() -> Self;

    fn blue() -> Self;

    fn white() -> Self;

    fn black() -> Self;

    fn transparent() -> Self;

    fn yellow() -> Self;

    fn cyan() -> Self;
}

impl ColorExt for Color {
    fn to_vec3(&self) -> Vec3 {
        Vec3::new(self.red, self.green, self.blue)
    }

    fn to_vec4(&self) -> Vec4 {
        Vec4::new(self.red, self.green, self.blue, self.alpha)
    }

    fn to_hsva(&self) -> Hsva {
        Hsva::from_color(*self)
    }

    fn to_hsla(&self) -> Hsla {
        Hsla::from_color(*self)
    }

    // TODO: replace with color scheme from violet
    fn red() -> Self {
        Color::new(1.0, 0.0, 0.0, 1.0)
    }

    fn green() -> Self {
        Color::new(0.0, 1.0, 0.0, 1.0)
    }

    fn blue() -> Self {
        Color::new(0.0, 0.0, 1.0, 1.0)
    }

    fn white() -> Self {
        Color::new(1.0, 1.0, 1.0, 1.0)
    }

    fn black() -> Self {
        Color::new(0.0, 0.0, 0.0, 1.0)
    }

    fn transparent() -> Self {
        Color::new(0.0, 0.0, 0.0, 0.0)
    }

    fn yellow() -> Self {
        Color::new(1.0, 1.0, 0.0, 1.0)
    }

    fn cyan() -> Self {
        Color::new(0.0, 1.0, 1.0, 1.0)
    }
}
// impl<'a> Lerp<'a> for Color {
//     type Write = &'a mut Color;

//     fn lerp(write: Self::Write, start: &Self, end: &Self, t: f32) {
//         *write = Color::from(Vec4::from(start).lerp(Vec4::from(end), t));
//     }
// }
