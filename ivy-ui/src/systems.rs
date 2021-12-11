use crate::{constraints::*, events::WidgetEvent, InteractiveState, Result};
use glfw::{Action, WindowEvent};
use hecs::{Entity, World};
use hecs_hierarchy::Hierarchy;
use hecs_schedule::{GenericWorld, Read, SubWorld, Write};
use ivy_base::{Events, Position2D, Size2D};
use ivy_graphics::Camera;
use ivy_input::InputEvent;
use ultraviolet::{Mat4, Vec2};

use crate::{constraints::ConstraintQuery, *};

/// Updates all UI trees and applies constraints.
/// Also updates canvas cameras.
pub fn update(world: &World) -> Result<()> {
    world.roots::<Widget>()?.iter().try_for_each(|(root, _)| {
        apply_constraints(world, root, Position2D::default(), Size2D::new(1.0, 1.0))?;

        if world.get::<Canvas>(root).is_ok() {
            update_canvas(world, root)?;
        }

        update_from(world, root, 1)
    })
}

pub(crate) fn update_from(world: &impl GenericWorld, parent: Entity, depth: u32) -> Result<()> {
    let mut query = world.try_query_one::<(&Position2D, &Size2D, &mut WidgetDepth)>(parent)?;
    let (position, size, curr_depth) = query.get()?;
    let position = *position;
    let size = *size;
    *curr_depth = depth.into();

    drop(query);

    if let Ok(layout) = world.try_get::<WidgetLayout>(parent) {
        layout.update(world, parent, position, size, depth)
    } else {
        world.children::<Widget>(parent).try_for_each(|child| {
            apply_constraints(world, child, position, size)?;
            update_from(world, child, depth + 1)
        })
    }
}

/// Applies the constraints associated to entity and uses the given parent.
pub(crate) fn apply_constraints(
    world: &impl GenericWorld,
    entity: Entity,
    parent_pos: Position2D,
    parent_size: Size2D,
) -> Result<()> {
    let mut constaints_query = world.try_query_one::<ConstraintQuery>(entity)?;
    let constraints = constaints_query.get()?;

    let mut query = world.try_query_one::<(&mut Position2D, &mut Size2D)>(entity)?;

    let (pos, size) = query.get()?;

    *size =
        constraints.rel_size.calculate(parent_size) + constraints.abs_size.calculate(parent_size);

    *pos = parent_pos
        + constraints.rel_offset.calculate(parent_size)
        + constraints.abs_offset.calculate(parent_size)
        - Position2D(**constraints.origin * **size);

    if **constraints.aspect != 0.0 {
        size.x = size.y * **constraints.aspect
    }

    Ok(())
}

/// Updates the canvas view and projection
pub fn update_canvas(world: &World, canvas: Entity) -> Result<()> {
    let mut camera_query = world.try_query_one::<(&mut Camera, &Size2D, &Position2D)>(canvas)?;

    let (camera, size, position) = camera_query.get()?;

    camera.set_orthographic(size.x * 2.0, size.y * 2.0, 0.0, 100.0);
    camera.set_view(Mat4::from_translation(-position.xyz()));

    Ok(())
}

pub fn reactive_system<T: 'static + Copy + Send + Sync, I: Iterator<Item = WidgetEvent>>(
    world: &World,
    events: I,
) -> Result<()> {
    events
        .filter_map(|event| ReactiveState::try_from_event(&event).map(|val| (event.entity(), val)))
        .try_for_each(|(entity, state)| -> Result<()> {
            eprintln!("Got: {:?}", state);
            let mut query = world.try_query_one::<(&mut T, &Reactive<T>)>(entity)?;
            let (val, reactive) = query.get()?;
            reactive.update(val, state);
            Ok(())
        })
}

pub fn handle_events(
    world: SubWorld<(&Position2D, &Size2D, &WidgetDepth, &Sticky)>,
    mut events: Write<Events>,
    mut state: Write<InteractiveState>,
    cursor_pos: Read<Position2D>,
    window_events: impl Iterator<Item = WindowEvent>,
    control_events: impl Iterator<Item = UIControl>,
) {
    control_events.for_each(|event| match event {
        UIControl::Unfocus => state.unfocus(&mut events),
        UIControl::Focus(entity) => state.set_focus(
            FocusedWidget::new(entity, world.try_get::<Sticky>(entity).is_ok()),
            &mut events,
        ),
    });

    window_events.for_each(|event| {
        let event = InputEvent::from(event);
        let hovered_widget = intersect_widget(&world, *cursor_pos);

        let event = match (event, hovered_widget, state.focused()) {
            // Mouse was clicked on a ui element
            (
                InputEvent::MouseButton {
                    button,
                    action: Action::Press,
                    mods,
                },
                Some(hovered_widget),
                _,
            ) => {
                events.send(WidgetEvent::new(
                    hovered_widget.id(),
                    InputEvent::MouseButton {
                        button,
                        action: Action::Press,
                        mods,
                    },
                ));

                // New focus, unfocus old
                if state.focused() != Some(&hovered_widget) {
                    state.set_focus(hovered_widget, &mut events);
                }

                None
            }
            // Mouse was clicked outside UI, lose focus
            (
                InputEvent::MouseButton {
                    button,
                    action: Action::Press,
                    mods,
                },
                None,
                Some(_),
            ) => {
                state.unfocus(&mut events);
                Some(InputEvent::MouseButton {
                    button,
                    action: Action::Press,
                    mods,
                })
            }
            (
                InputEvent::MouseButton {
                    button,
                    action: Action::Release,
                    mods,
                },
                hovered_widget,
                Some(widget),
            ) => {
                // Mouse was released on the same widget
                if hovered_widget == Some(*widget) {
                    events.send(WidgetEvent::new(
                        widget.id(),
                        InputEvent::MouseButton {
                            button,
                            action: Action::Release,
                            mods,
                        },
                    ));
                }
                // Send unfocus event if widget is not sticky
                if !widget.sticky() {
                    state.unfocus(&mut events);
                }

                None
            }
            // If a widget is focused and all else was handled, forward all events
            (event, _, Some(widget)) => {
                events.send(WidgetEvent::new(widget.id(), event));
                None
            }

            (event, _, _) => Some(event),
        };

        if let Some(event) = event {
            events.send(event);
        }
    })
}

/// Returns the first widget that intersects the postiion
fn intersect_widget(world: &impl GenericWorld, point: Position2D) -> Option<FocusedWidget> {
    world
        .try_query::<(&Position2D, &Size2D, &WidgetDepth)>()
        .unwrap()
        .with::<Interactive>()
        .iter()
        .filter_map(|(e, (pos, size, depth))| {
            if box_intersection(*pos, *size, *point) {
                Some((e, *depth))
            } else {
                None
            }
        })
        .max_by_key(|(_, depth)| *depth)
        .map(|(e, _)| FocusedWidget::new(e, world.try_get::<Sticky>(e).ok().is_some()))
}

fn box_intersection(pos: Position2D, size: Size2D, point: Vec2) -> bool {
    let pos = *pos;
    let size = *size;

    point.x > pos.x - size.x
        && point.x < pos.x + size.x
        && point.y > pos.y - size.y
        && point.y < pos.y + size.y
}
