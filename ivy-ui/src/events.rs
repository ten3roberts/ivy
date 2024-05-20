//! This module abstracts the raw window events into input events.
//! Window events are captured and forwarded to input events if no UI element
//! captured it.

use std::convert::TryFrom;

use flax::Entity;
use glam::Vec2;
use glfw::{Action, Key, Modifiers, MouseButton, Scancode};

/// Event for a clicked ui widget
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetEvent {
    pub entity: Entity,
    pub kind: WidgetEventKind,
}

impl WidgetEvent {
    pub fn new(entity: Entity, kind: WidgetEventKind) -> Self {
        Self { entity, kind }
    }

    /// Get a reference to the widget event's entity.
    #[inline]
    pub fn entity(&self) -> Entity {
        self.entity
    }

    /// Get a reference to the widget event's kind.
    #[inline]
    pub fn kind(&self) -> &WidgetEventKind {
        &self.kind
    }
}

/// Events to control the UI layer
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UIControl {
    /// Set or release focus to the given entity
    Focus(Option<Entity>),
}

/// A subset of input events for UI widgets
#[derive(Debug, Clone, PartialEq)]
pub enum WidgetEventKind {
    CursorPos(Vec2),
    Key {
        key: Key,
        scancode: Scancode,
        action: Action,
        mods: Modifiers,
    },

    /// Scroll wheel event in horizontal and vertical
    Scroll(Vec2),
    /// A typed char with applied modifiers
    CharTyped(char),
    CharModifiers {
        c: char,
        mods: Modifiers,
    },

    /// Explicit focus given or lost
    Focus(bool),
    /// Cursor is above
    Hover(bool),

    /// Widget was clicked
    /// Usually sent at the same time as focus
    MouseButton {
        button: MouseButton,
        action: Action,
        mods: Modifiers,
    },
}

// impl TryFrom<InputEvent> for WidgetEventKind {
//     type Error = InputEvent;

//     fn try_from(value: InputEvent) -> Result<Self, Self::Error> {
//         match value {
//             InputEvent::Key {
//                 key,
//                 scancode,
//                 action,
//                 mods,
//             } => Ok(Self::Key {
//                 key,
//                 scancode,
//                 action,
//                 mods,
//             }),
//             InputEvent::CursorPos(val) => Ok(Self::CursorPos(val)),
//             InputEvent::Scroll(val) => Ok(Self::Scroll(val)),
//             InputEvent::CharTyped(val) => Ok(Self::CharTyped(val)),
//             InputEvent::CharModifiers { c, mods } => Ok(Self::CharModifiers { c, mods }),
//             InputEvent::Focus(val) => Ok(Self::Focus(val)),
//             InputEvent::MouseButton {
//                 button,
//                 action,
//                 mods,
//             } => Ok(Self::MouseButton {
//                 button,
//                 action,
//                 mods,
//             }),
//             other => Err(other),
//         }
//     }
// }
