use std::convert::TryInto;

use crate::{constraints::*, events::WidgetEvent, InteractiveState, Result};
use glam::{Mat4, Vec2};
use glfw::{Action, WindowEvent};
use hecs::{Entity, World};
use hecs_hierarchy::Hierarchy;
use hecs_schedule::{GenericWorld, Read, Write};
use ivy_base::{Events, Position2D, Size2D, Visible};
use ivy_graphics::Camera;
use ivy_input::InputEvent;

use crate::{constraints::ConstraintQuery, *};

/// Updates all UI trees and applies constraints.
/// Also updates canvas cameras.
pub fn update(world: &World) -> Result<()> {
    world.roots::<Widget>()?.iter().try_for_each(|(root, _)| {
        apply_constraints(
            world,
            root,
            Position2D::default(),
            Size2D::new(1.0, 1.0),
            true,
        )?;

        if world.get::<Canvas>(root).is_ok() {
            update_canvas(world, root)?;
        }

        update_from(world, root, 1)
    })
}

pub(crate) fn update_from(world: &impl GenericWorld, parent: Entity, depth: u32) -> Result<()> {
    let mut query =
        world.try_query_one::<(&Position2D, &Size2D, &mut WidgetDepth, &mut Visible)>(parent)?;
    let (position, size, curr_depth, visible) = query.get()?;
    let position = *position;
    let size = *size;
    *curr_depth = depth.into();

    let is_visible = visible.is_visible();

    drop(query);

    if let Ok(layout) = world.try_get::<WidgetLayout>(parent) {
        layout.update(world, parent, position, size, depth, is_visible)
    } else {
        world.children::<Widget>(parent).try_for_each(|child| {
            apply_constraints(world, child, position, size, is_visible)?;
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
    is_visible: bool,
) -> Result<()> {
    let mut constaints_query =
        world.try_query_one::<(ConstraintQuery, &mut Position2D, &mut Size2D, &mut Visible)>(
            entity,
        )?;
    let (constraints, pos, size, visible) = constaints_query.get()?;

    if !is_visible {
        *visible = Visible::HiddenInherit;
    } else if *visible == Visible::HiddenInherit {
        *visible = Visible::Visible;
    }

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
    camera.set_view(Mat4::from_translation(-position.extend(0.0)));

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
            if let Ok((val, reactive)) = query.get() {
                reactive.update(val, state);
            }
            Ok(())
        })
}

pub fn handle_events(
    world: Write<World>,
    mut events: Write<Events>,
    mut state: Write<InteractiveState>,
    cursor_pos: Read<Position2D>,
    window_events: impl Iterator<Item = WindowEvent>,
    control_events: impl Iterator<Item = UIControl>,
) {
    control_events.for_each(|event| match event {
        UIControl::Focus(widget) => state.set_focus(widget, true, &mut events),
    });

    let hovered = intersect_widget(&*world, *cursor_pos);

    let sticky = hovered
        .map(|val| world.get::<Sticky>(val).is_ok())
        .unwrap_or_default();

    window_events.for_each(|event| {
        let event = InputEvent::from(event);

        state.set_hovered(hovered, &mut events);

        let event = match event {
            // Mouse was clicked on a ui element
            InputEvent::MouseButton {
                button,
                action: Action::Press,
                mods,
            } => {
                state.set_focus(hovered, sticky, &mut events);

                // Swallow or forward event
                if let Some(widget) = hovered {
                    let entity = world.entity(widget).unwrap();

                    if let Some(click) = entity.get::<OnClick>() {
                        click.0(entity, &mut events);
                    }

                    events.send(WidgetEvent::new(
                        widget,
                        WidgetEventKind::MouseButton {
                            button,
                            action: Action::Press,
                            mods,
                        },
                    ));

                    None
                } else {
                    Some(InputEvent::MouseButton {
                        button,
                        action: Action::Press,
                        mods,
                    })
                }
            }
            InputEvent::MouseButton {
                button,
                action: Action::Release,
                mods,
            } if state.focused().is_some() => {
                // Mouse was released on the same widget
                if let Some(hovered) = hovered {
                    if Some(hovered) == state.focused() {
                        events.send(WidgetEvent::new(
                            hovered,
                            WidgetEventKind::MouseButton {
                                button,
                                action: Action::Release,
                                mods,
                            },
                        ));
                    }
                }

                // Send unfocus event if widget is not sticky
                dbg!("Sticky: {:?}", state.sticky());
                if !state.sticky() {
                    state.set_focus(None, false, &mut events);
                }

                None
            }
            // If a widget is focused and all else was handled, forward all events
            event if state.focused().is_some() => match event.try_into() {
                Ok(val) => {
                    events.send(WidgetEvent::new(state.focused().unwrap(), val));
                    None
                }
                Err(val) => Some(val),
            },

            event => Some(event),
        };

        if let Some(event) = event {
            events.send(event);
        }
    })
}

/// Returns the first widget that intersects the postiion
fn intersect_widget(world: &impl GenericWorld, point: Position2D) -> Option<Entity> {
    world
        .try_query::<(&Position2D, &Size2D, &WidgetDepth, &Visible)>()
        .unwrap()
        .with::<Interactive>()
        .iter()
        .filter_map(|(e, (pos, size, depth, visible))| {
            if visible.is_visible() && box_intersection(*pos, *size, *point) {
                Some((e, depth))
            } else {
                None
            }
        })
        .max_by_key(|(_, depth)| *depth)
        .map(|(a, _)| a)
}

fn box_intersection(pos: Position2D, size: Size2D, point: Vec2) -> bool {
    let pos = *pos;
    let size = *size;

    point.x > pos.x - size.x
        && point.x < pos.x + size.x
        && point.y > pos.y - size.y
        && point.y < pos.y + size.y
}
