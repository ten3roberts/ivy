use std::collections::BTreeSet;

use flax::component;
use glam::Vec2;
use winit::{event::Modifiers, keyboard::Key};

use crate::InputState;

component! {
    pub input_state: InputState,

    // Normalized cursor position on the active window.
    pub cursor_position: Vec2,
}
