use ultraviolet::Vec3;

use crate::{Input, InputAxis};

/// An input vector is a collection of `InputAxis` for each cardinal direction
pub struct InputVector {
    pub x: InputAxis,
    pub y: InputAxis,
    pub z: InputAxis,
}

impl InputVector {
    pub fn new(x: InputAxis, y: InputAxis, z: InputAxis) -> Self {
        Self { x, y, z }
    }

    pub fn get(&self, input: &Input) -> Vec3 {
        Vec3 {
            x: self.x.get(input),
            y: self.y.get(input),
            z: self.z.get(input),
        }
    }
}
