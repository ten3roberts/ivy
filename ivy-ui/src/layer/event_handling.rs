use hecs::Entity;
use ivy_base::Events;
use ivy_input::InputEvent;

use crate::events::WidgetEvent;

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
    focused_widget: Option<FocusedWidget>,
}

impl InteractiveState {
    /// Properly removes focus from an item
    pub fn unfocus(&mut self, events: &mut Events) {
        if let Some(focused_widget) = self.focused_widget {
            events.send(WidgetEvent::new(
                focused_widget.id,
                InputEvent::Focus(false),
            ));
        }
        self.focused_widget = None
    }

    pub fn set_focus(&mut self, widget: FocusedWidget, events: &mut Events) {
        self.unfocus(events);

        events.send(WidgetEvent::new(widget.id, InputEvent::Focus(true)));
        self.focused_widget = Some(widget);
    }

    pub fn focused(&self) -> Option<&FocusedWidget> {
        self.focused_widget.as_ref()
    }
}
