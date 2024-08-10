use glam::{vec3, Vec3, Vec4};
pub use palette;
use palette::{FromColor, Hsla, Hsva, IntoColor, Srgb, Srgba};

pub type Color = Srgba;

pub trait ColorExt {
    fn to_vec3(&self) -> Vec3;
    fn to_vec4(&self) -> Vec4;
    fn to_hsva(&self) -> Hsva;
    fn to_hsla(&self) -> Hsla;

    fn from_hsla(h: f32, s: f32, l: f32, a: f32) -> Self;

    fn from_hsva(h: f32, s: f32, l: f32, a: f32) -> Self;

    fn red() -> Self;

    fn green() -> Self;

    fn blue() -> Self;

    fn white() -> Self;

    fn black() -> Self;

    fn transparent() -> Self;

    fn yellow() -> Self;

    fn cyan() -> Self;

    fn purple() -> Self;
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

    fn from_hsla(h: f32, s: f32, l: f32, a: f32) -> Self {
        Hsla::new(h, s, l, a).into_color()
    }

    fn from_hsva(h: f32, s: f32, v: f32, a: f32) -> Self {
        Hsva::new(h, s, v, a).into_color()
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

    fn purple() -> Self {
        Color::new(1.0, 0.0, 1.0, 1.0)
    }
}

pub fn to_linear_vec3(color: Srgb) -> Vec3 {
    let color = palette::rgb::LinSrgb::from_color(color);
    vec3(color.red, color.green, color.blue)
}
