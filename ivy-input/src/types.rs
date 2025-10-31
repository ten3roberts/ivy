use glam::Vec2;
use ivy_core::layer::events::Event;
use winit::{dpi::LogicalPosition, event::Modifiers, keyboard::SmolStr};
pub use winit::{
    event::{ElementState, MouseButton},
    keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey},
};

#[derive(Debug, Clone)]
pub enum InputEvent {
    Keyboard(KeyboardInput),
    ModifiersChanged(Modifiers),
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
    pub physical_key: PhysicalKey,
    pub state: ElementState,
    pub text: Option<SmolStr>,
}

#[derive(Debug, Clone)]
pub struct MouseMotion {
    pub delta: Vec2,
}

#[derive(Debug, Clone)]
pub struct ScrollMotion {
    pub delta: Vec2,
    pub line_delta: Vec2,
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
    Key(KeyCode),
    MouseButton(MouseButton),
    Modifiers,
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
            InputEvent::Keyboard(v) => match v.physical_key {
                PhysicalKey::Code(code) => InputKind::Key(code),
                _ => InputKind::Key(KeyCode::F1), // dummy for unidentified
            },
            InputEvent::MouseButton(v) => InputKind::MouseButton(v.button),
            InputEvent::ModifiersChanged(_) => InputKind::Modifiers,
            InputEvent::CursorMoved(_) => InputKind::CursorMoved,
            InputEvent::CursorDelta(_) => InputKind::CursorDelta,
            InputEvent::Scroll(_) => InputKind::Scroll,
            InputEvent::CursorLeft => InputKind::CursorLeft,
            InputEvent::CursorEntered => InputKind::CursorEntered,
            InputEvent::Focus(_) => InputKind::Focus,
        }
    }
}
