use glam::Vec2;
use ivy_base::layer::events::Event;
use winit::{
    event::{ElementState, MouseButton},
    keyboard::{Key, ModifiersState},
};

#[derive(Debug, Clone)]
pub struct KeyboardInput {
    pub modifiers: ModifiersState,
    pub key: Key,
    pub state: ElementState,
}

#[derive(Debug, Clone)]
pub struct CursorMoved {
    pub position: Vec2,
}

#[derive(Debug, Clone)]
pub struct MouseInput {
    pub modifiers: ModifiersState,
    pub button: MouseButton,
    pub state: ElementState,
}

impl Event for CursorMoved {}
impl Event for KeyboardInput {}
impl Event for MouseInput {}
