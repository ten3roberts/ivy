use std::ops::Deref;

use glam::Vec3;
use glfw::Key;

use crate::{Input, InputAxis};

/// An input vector is a collection of `InputAxis` for each cardinal direction
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct InputVector {
    pub x: InputAxis,
    pub y: InputAxis,
    pub z: InputAxis,
}

impl InputVector {
    pub fn new(x: InputAxis, y: InputAxis, z: InputAxis) -> Self {
        Self { x, y, z }
    }

    pub fn wasd() -> Self {
        Self {
            x: InputAxis::keyboard(Key::A, Key::D),
            y: InputAxis::none(),
            z: InputAxis::keyboard(Key::W, Key::S),
        }
    }

    /// Returns the value of the input vector based on the current input state.
    pub fn get(&self, input: &impl Deref<Target = Input>) -> Vec3 {
        Vec3::new(self.x.get(input), self.y.get(input), self.z.get(input))
    }
}
