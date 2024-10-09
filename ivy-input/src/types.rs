use glam::Vec2;
use ivy_core::layer::events::Event;
use winit::dpi::LogicalPosition;
pub use winit::{
    event::{ElementState, MouseButton},
    keyboard::{Key, ModifiersState, NamedKey},
};

#[derive(Debug, Clone)]
pub struct KeyboardInput {
    pub modifiers: ModifiersState,
    pub key: Key,
    pub state: ElementState,
}

#[derive(Debug, Clone)]
pub struct MouseMotion {
    pub delta: Vec2,
}

#[derive(Debug, Clone)]
pub struct ScrollMotion {
    pub delta: Vec2,
}

#[derive(Debug, Clone)]
pub struct MouseInput {
    pub modifiers: ModifiersState,
    pub button: MouseButton,
    pub state: ElementState,
}

#[derive(Debug, Clone, Copy)]
pub struct CursorMoved {
    pub absolute_position: LogicalPosition<f32>,
    pub normalized_position: Vec2,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum InputKind {
    Key(Key),
    MouseButton(MouseButton),
    CursorMoved,
    CursorDelta,
    Scroll,
}

#[derive(Debug, Clone)]
pub enum InputEvent {
    Key(KeyboardInput),
    MouseButton(MouseInput),
    CursorMoved(CursorMoved),
    CursorDelta(Vec2),
    Scroll(Vec2),
}

#[derive(Debug, Clone)]
pub struct CursorLeft;
#[derive(Debug, Clone)]
pub struct CursorEntered;

impl Event for KeyboardInput {}
impl Event for MouseInput {}
impl Event for CursorMoved {}
impl Event for MouseMotion {}
impl Event for ScrollMotion {}

impl Event for CursorLeft {}
impl Event for CursorEntered {}

impl InputEvent {
    pub(crate) fn to_kind(&self) -> InputKind {
        match self {
            InputEvent::Key(v) => InputKind::Key(v.key.clone()),
            InputEvent::MouseButton(v) => InputKind::MouseButton(v.button),
            InputEvent::CursorMoved(_) => InputKind::CursorMoved,
            InputEvent::CursorDelta(_) => InputKind::CursorDelta,
            InputEvent::Scroll(_) => InputKind::Scroll,
        }
    }
}
