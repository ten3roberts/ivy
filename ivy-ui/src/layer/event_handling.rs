use glfw::{Action, WindowEvent};
use hecs::{Entity, World};
use ivy_base::{Events, Position2D, Size2D};
use ivy_input::InputEvent;
use ultraviolet::Vec2;

use crate::{events::WidgetEvent, Interactive, WidgetDepth};

pub const MAX_BUTTON: usize = glfw::MouseButton::Button8 as usize;

/// Holds interactive status such as clicked widget and dragging etc.
#[derive(Default, Debug, Clone, PartialEq, Eq)]
pub struct InteractiveState {
    pressed: [Option<Entity>; MAX_BUTTON],
    /// The currently selected widget
    active_widget: Option<Entity>,
}

pub fn handle_events<I: Iterator<Item = WindowEvent>>(
    world: &World,
    events: &mut Events,
    window_events: I,
    cursor_pos: Position2D,
    state: &mut InteractiveState,
) {
    window_events.for_each(|val| {
        let event = InputEvent::from(val);
        let event = match event {
            // Absorb
            InputEvent::Key {
                key: _,
                scancode: _,
                action: _,
                mods: _,
            } if state.active_widget.is_some() => None,
            InputEvent::CharTyped(c) => {
                if let Some(widget) = state.active_widget {
                    WidgetEvent::CharTyped(widget, c);
                    None
                } else {
                    Some(event)
                }
            }
            InputEvent::MouseButton {
                button,
                action: Action::Press,
                mods: _,
            } => {
                state.active_widget = intersect_widget(world, cursor_pos);
                if let Some(widget) = state.active_widget {
                    state.pressed[button as usize] = Some(widget);
                    events.send(WidgetEvent::Pressed(widget, button));
                    None
                } else {
                    Some(event)
                }
            }
            // Only absorb click if the press started on a widget regardless of it was
            // released on a widget
            InputEvent::MouseButton {
                button,
                action: Action::Release,
                mods: _,
            } if state.pressed[button as usize].is_some() => {
                let current_widget = intersect_widget(world, cursor_pos);
                if current_widget == state.pressed[button as usize] {
                    if let Some(widget) = current_widget {
                        events.send(WidgetEvent::Released(widget, button));
                    }
                }

                state.pressed[button as usize] = None;
                None
            }
            event => Some(event),
        };

        if let Some(event) = event {
            events.send(event);
        }
    })
}

/// Returns the first widget that intesects the postiion
fn intersect_widget(world: &World, point: Position2D) -> Option<Entity> {
    world
        .query::<(&Position2D, &Size2D, &WidgetDepth)>()
        .with::<Interactive>()
        .iter()
        .filter_map(|(e, (pos, size, depth))| {
            eprintln!("Looking at: {:?}", e);
            if box_intersection(*pos, *size, *point) {
                Some((e, *depth))
            } else {
                None
            }
        })
        .max_by_key(|(_, depth)| *depth)
        .map(|(e, _)| e)
}

fn box_intersection(pos: Position2D, size: Size2D, point: Vec2) -> bool {
    let pos = *pos;
    let size = *size;
    eprintln!("Point: {:?}, box: {:?}: {:?}", point, pos, size);

    point.x > pos.x - size.x
        && point.x < pos.x + size.x
        && point.y > pos.y - size.y
        && point.y < pos.y + size.y
}
