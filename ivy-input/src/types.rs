use glam::Vec2;
use ivy_core::layer::events::Event;
use winit::dpi::LogicalPosition;
pub use winit::{
    event::{ElementState, MouseButton},
    keyboard::{Key, ModifiersState, NamedKey},
};

#[derive(Debug, Clone)]
pub enum InputEvent {
    Keyboard(KeyboardInput),
    Scroll(ScrollMotion),
    MouseButton(MouseInput),
    CursorMoved(CursorMoved),
    CursorDelta(Vec2),
    CursorLeft,
    CursorEntered,
    Focus(bool),
}

impl Event for InputEvent {}

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
    CursorLeft,
    CursorEntered,
    Focus,
}

#[derive(Debug, Clone)]
pub struct CursorLeft;
#[derive(Debug, Clone)]
pub struct CursorEntered;

impl InputEvent {
    pub(crate) fn to_kind(&self) -> InputKind {
        match self {
            InputEvent::Keyboard(v) => InputKind::Key(v.key.clone()),
            InputEvent::MouseButton(v) => InputKind::MouseButton(v.button),
            InputEvent::CursorMoved(_) => InputKind::CursorMoved,
            InputEvent::CursorDelta(_) => InputKind::CursorDelta,
            InputEvent::Scroll(_) => InputKind::Scroll,
            InputEvent::CursorLeft => InputKind::CursorLeft,
            InputEvent::CursorEntered => InputKind::CursorEntered,
            InputEvent::Focus(_) => InputKind::Focus,
        }
    }
}
