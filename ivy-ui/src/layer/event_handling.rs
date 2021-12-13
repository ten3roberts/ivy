use hecs::Entity;
use ivy_base::Events;

use crate::{events::WidgetEvent, WidgetEventKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FocusedWidget {
    id: Entity,
    sticky: bool,
}

impl FocusedWidget {
    pub fn new(id: Entity, sticky: bool) -> Self {
        Self { id, sticky }
    }

    /// Get a reference to the active widget's id.
    #[inline]
    pub fn id(&self) -> Entity {
        self.id
    }

    /// Get a reference to the active widget's sticky.
    #[inline]
    pub fn sticky(&self) -> bool {
        self.sticky
    }
}

/// Holds interactive status such as clicked widget and dragging etc.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct InteractiveState {
    /// The currently selected widget and sticky status
    focused: Option<Entity>,
    sticky: bool,
    hovered: Option<Entity>,
}

impl InteractiveState {
    /// Properly removes focus from an item
    fn unfocus(&mut self, events: &mut Events) {
        if let Some(focused_widget) = self.focused {
            events.send(WidgetEvent::new(
                focused_widget,
                WidgetEventKind::Focus(false),
            ));
        }
        self.focused = None
    }

    /// Set focus with appropriate events
    /// Idempotent
    pub fn set_focus(&mut self, widget: Option<Entity>, sticky: bool, events: &mut Events) {
        // Idempotent
        if widget == self.focused {
            return;
        }

        self.unfocus(events);
        self.sticky = sticky;

        if let Some(widget) = widget {
            events.send(WidgetEvent::new(widget, WidgetEventKind::Focus(true)));
            self.focused = Some(widget);
        }
    }

    pub fn focused(&self) -> Option<Entity> {
        self.focused
    }

    pub fn hovered(&self) -> Option<Entity> {
        self.hovered
    }

    fn remove_hover(&mut self, events: &mut Events) {
        if let Some(hovered) = self.hovered {
            events.send(WidgetEvent::new(hovered, WidgetEventKind::Hover(false)));
            self.hovered = None;
        }
    }

    /// Set hover with appropriate events
    /// Idempotent
    pub fn set_hovered(&mut self, widget: Option<Entity>, events: &mut Events) {
        if widget == self.hovered {
            return;
        }

        self.remove_hover(events);
        if let Some(widget) = widget {
            events.send(WidgetEventKind::Hover(true));
            self.hovered = Some(widget);
        }
    }

    /// Get a reference to the interactive state's sticky.
    pub fn sticky(&self) -> bool {
        self.sticky
    }
}
