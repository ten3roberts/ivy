use glam::Vec2;
use ivy_base::layer::events::Event;
pub use winit::{
    event::{ElementState, MouseButton},
    keyboard::{Key, ModifiersState},
};

use crate::InputKind;

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

#[derive(Debug, Clone)]
pub enum InputEvent {
    Key(KeyboardInput),
    MouseButton(MouseInput),
    CursorMoved(Vec2),
}

#[derive(Debug, Clone)]
pub struct CursorLeft;
#[derive(Debug, Clone)]
pub struct CursorEntered;

#[derive(Debug, Clone)]
pub struct ScrollInput {
    pub delta: Vec2,
}

impl Event for KeyboardInput {}
impl Event for MouseInput {}
impl Event for CursorMoved {}
impl Event for ScrollInput {}

impl Event for CursorLeft {}
impl Event for CursorEntered {}

impl InputEvent {
    pub(crate) fn to_kind(&self) -> InputKind {
        match self {
            InputEvent::Key(v) => InputKind::Key(v.key.clone()),
            InputEvent::MouseButton(v) => InputKind::MouseButton(v.button),
            InputEvent::CursorMoved(_) => InputKind::CursorMoved,
        }
    }
}
