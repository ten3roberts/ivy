use glam::{vec2, vec3, vec4, Vec2, Vec3, Vec4};

/// Returns the inverse of a type.
/// Can be used to safely divide by a number and return 0 instead of Nan
pub trait Inverse {
    type Output;
    fn inv(&self) -> Self::Output;
}

impl Inverse for f32 {
    type Output = f32;

    fn inv(&self) -> Self::Output {
        if self.is_normal() {
            1.0 / self
        } else {
            0.0
        }
    }
}

impl Inverse for f64 {
    type Output = f64;

    fn inv(&self) -> Self::Output {
        if self.is_normal() {
            1.0 / self
        } else {
            0.0
        }
    }
}

impl Inverse for Vec2 {
    type Output = Vec2;

    fn inv(&self) -> Self::Output {
        vec2(self.x.inv(), self.y.inv())
    }
}

impl Inverse for Vec3 {
    type Output = Vec3;

    fn inv(&self) -> Self::Output {
        vec3(self.x.inv(), self.y.inv(), self.z.inv())
    }
}

impl Inverse for Vec4 {
    type Output = Vec4;

    fn inv(&self) -> Self::Output {
        vec4(self.x.inv(), self.y.inv(), self.z.inv(), self.w.inv())
    }
}
