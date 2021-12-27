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
