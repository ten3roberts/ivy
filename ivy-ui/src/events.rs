//! This module abstracts the raw window events into input events.
//! Window events are captured and forwarded to input events if no UI element
//! captured it.

use hecs::Entity;
use ivy_input::InputEvent;

/// Event for a clicked ui widget
#[derive(Debug, Clone, PartialEq)]
pub struct WidgetEvent {
    pub entity: Entity,
    pub kind: InputEvent,
}

impl WidgetEvent {
    pub fn new(entity: Entity, kind: InputEvent) -> Self {
        Self { entity, kind }
    }

    /// Get a reference to the widget event's entity.
    #[inline]
    pub fn entity(&self) -> Entity {
        self.entity
    }

    /// Get a reference to the widget event's kind.
    #[inline]
    pub fn kind(&self) -> &InputEvent {
        &self.kind
    }
}

/// Events to control the UI layer
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UIControl {
    /// Release focus of the currently focused entity
    Unfocus,
    /// Set focus to the given entity
    Focus(Entity),
}
