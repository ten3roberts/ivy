use glfw::{Action, Key, MouseButton, Window};

use crate::Input;

pub enum InputDirection {
    Horizontal,
    Vertical,
}

pub enum InputAxis {
    Keyboard {
        pos: Key,
        neg: Key,
    },
    MouseButton {
        pos: MouseButton,
        neg: MouseButton,
    },
    Scroll {
        invert: bool,
        dir: InputDirection,
    },
    /// Represents a dummy, always 0 input axis
    None,
}

impl InputAxis {
    pub fn keyboard(pos: Key, neg: Key) -> Self {
        InputAxis::Keyboard { pos, neg }
    }

    pub fn mouse_button(pos: MouseButton, neg: MouseButton) -> Self {
        InputAxis::MouseButton { pos, neg }
    }

    pub fn scroll(dir: InputDirection, invert: bool) -> Self {
        InputAxis::Scroll { dir, invert }
    }

    pub fn none() -> Self {
        InputAxis::None
    }

    /// Gets the current value of the axis from the input state.
    pub fn get(&self, input: &Input) -> f32 {
        match self {
            InputAxis::Keyboard { pos, neg } => {
                (input.key(*pos) as i32 as f32) - (input.key(*neg) as i32 as f32)
            }
            InputAxis::MouseButton { pos, neg } => {
                (input.mouse_button(*pos) as i32 as f32) - (input.mouse_button(*neg) as i32 as f32)
            }
            InputAxis::Scroll {
                dir: InputDirection::Horizontal,
                invert,
            } => input.scroll().x * if *invert { -1.0 } else { 1.0 },
            InputAxis::Scroll {
                dir: InputDirection::Vertical,
                invert,
            } => input.scroll().y * if *invert { -1.0 } else { 1.0 },
            InputAxis::None => 0.0,
        }
    }
}
