//! This module abstracts the raw window events into input events.
//! Window events are captured and forwarded to input events if no UI element
//! captured it.

use glfw::MouseButton;
use hecs::Entity;

/// Event for a clicked ui widget
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WidgetEvent {
    /// The mouse button was pressed down on this widget
    Pressed(Entity, MouseButton),
    /// The mouse button was pressed down and subsequently released on this
    /// widget
    Released(Entity, MouseButton),
    /// A character was pressed while this widget was active
    CharTyped(Entity, char),
}
