use std::ops::Deref;

use glfw::Key;
use ultraviolet::Vec3;

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
            x: InputAxis::keyboard(Key::D, Key::A),
            y: InputAxis::none(),
            z: InputAxis::keyboard(Key::S, Key::W),
        }
    }

    /// Returns the value of the input vector based on the current input state.
    pub fn get(&self, input: &impl Deref<Target = Input>) -> Vec3 {
        Vec3 {
            x: self.x.get(input),
            y: self.y.get(input),
            z: self.z.get(input),
        }
    }
}
