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
}
// impl<'a> Lerp<'a> for Color {
//     type Write = &'a mut Color;

//     fn lerp(write: Self::Write, start: &Self, end: &Self, t: f32) {
//         *write = Color::from(Vec4::from(start).lerp(Vec4::from(end), t));
//     }
// }
