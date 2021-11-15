//! This module abstracts the raw window events into input events.
//! Window events are captured and forwarded to input events if no UI element
//! captured it.

use glfw::MouseButton;
use hecs::Entity;

/// Event for a clicked ui widget
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WidgetEvent {
    entity: Entity,
    kind: WidgetEventKind,
}

impl WidgetEvent {
    pub fn new(entity: Entity, kind: WidgetEventKind) -> Self {
        Self { entity, kind }
    }

    /// Get a reference to the widget event's entity.
    pub fn entity(&self) -> Entity {
        self.entity
    }

    /// Get a reference to the widget event's kind.
    pub fn kind(&self) -> &WidgetEventKind {
        &self.kind
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WidgetEventKind {
    /// The mouse button was pressed down on this widget
    Pressed(MouseButton),
    /// The mouse button was pressed down and subsequently released on this
    /// widget
    Released(MouseButton),
    /// A character was pressed while this widget was active
    CharTyped(char),
    /// Mouse button was released after pressing a widget but the cursor was no
    /// longer on the same widget.
    ReleasedBackground(MouseButton),
}
