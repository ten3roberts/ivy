use crate::{
    constraints::{UIOffset, UISize},
    events::WidgetEvent,
    InteractiveState, Result,
};
use glfw::{Action, WindowEvent};
use hecs::{Entity, World};
use hecs_hierarchy::Hierarchy;
use ivy_base::{Color, Events, Position2D, Size2D};
use ivy_graphics::Camera;
use ivy_input::InputEvent;
use ultraviolet::{Mat4, Vec2};

use crate::{constraints::ConstraintQuery, *};

/// Updates all UI trees and applies constraints.
/// Also updates canvas cameras.
pub fn update(world: &World) -> Result<()> {
    world.roots::<Widget>().iter().try_for_each(|(root, _)| {
        if world.get::<Canvas>(root).is_ok() {
            update_canvas(world, root)?;
        }

        update_from(world, root, 1)
    })
}

pub fn update_from(world: &World, parent: Entity, depth: u32) -> Result<()> {
    let mut query = world.query_one::<(&Position2D, &Size2D, Option<&mut WidgetDepth>)>(parent)?;
    let mut results = query.get().ok_or(hecs::NoSuchEntity)?;
    let position = *results.0;
    let size = *results.1;

    match results.2.as_mut() {
        Some(val) => **val = depth.into(),
        None => {}
    };

    drop(results);
    drop(query);

    world.children::<Widget>(parent).try_for_each(|child| {
        apply_constraints(world, child, position, size)?;
        assert!(parent != child);
        update_from(world, child, depth + 1)
    })
}

/// Applies the constaints associated to entity and uses the given parent.
fn apply_constraints(
    world: &World,
    entity: Entity,
    parent_pos: Position2D,
    parent_size: Size2D,
) -> Result<()> {
    let mut constaints_query = world.query_one::<ConstraintQuery>(entity)?;
    let constraints = constaints_query.get().ok_or(hecs::NoSuchEntity)?;

    let mut query = world.query_one::<(&mut Position2D, &mut Size2D)>(entity)?;

    let (pos, size) = query.get().ok_or(hecs::NoSuchEntity)?;

    *pos = parent_pos
        + constraints
            .rel_offset
            .map(|val| val.calculate(parent_size))
            .unwrap_or_default()
        + constraints
            .abs_offset
            .map(|val| val.calculate(parent_size))
            .unwrap_or_default();

    *size = constraints
        .rel_size
        .map(|val| val.calculate(parent_size))
        .unwrap_or_default()
        + constraints
            .abs_size
            .map(|val| val.calculate(parent_size))
            .unwrap_or_default()
        + constraints
            .offset_size
            .map(|val| val.calculate(parent_size))
            .unwrap_or_default();

    if let Some(aspect) = constraints.aspect {
        size.x = size.y * **aspect
    }

    Ok(())
}

/// Updates the canvas view and projection
pub fn update_canvas(world: &World, canvas: Entity) -> Result<()> {
    let mut camera_query =
        world.query_one::<(&mut Camera, &Size2D, Option<&Position2D>)>(canvas)?;

    let (camera, size, position) = camera_query.get().ok_or(hecs::NoSuchEntity)?;
    let position = *position.unwrap_or(&Position2D::default());

    camera.set_orthographic(size.x * 2.0, size.y * 2.0, 0.0, 100.0);
    camera.set_view(Mat4::from_translation(-position.xyz()));

    Ok(())
}

/// Satisfies all widget by adding missing ModelMatrices, Position2D and Size2D
pub fn statisfy_widgets(world: &mut World) {
    let entities = world
        .query_mut::<(&Widget, Option<&Color>)>()
        .into_iter()
        .map(|(e, (_, color))| (e, color.cloned().unwrap_or_default()))
        .collect::<Vec<_>>();

    entities.into_iter().for_each(|(e, color)| {
        // Ignore errors, we just collected these entities and thus know they exist.
        let _ = world.insert(
            e,
            (
                Position2D::default(),
                Size2D::default(),
                WidgetDepth::default(),
                color,
            ),
        );
    });
}

pub fn reactive_system<T: 'static + Copy + Send + Sync, I: Iterator<Item = WidgetEvent>>(
    world: &World,
    events: I,
) -> Result<()> {
    events
        .filter_map(|event| ReactiveState::try_from_event(&event).map(|val| (event.entity(), val)))
        .try_for_each(|(entity, state)| -> Result<()> {
            eprintln!("Got: {:?}", state);
            let mut query = world.query_one::<(&mut T, &Reactive<T>)>(entity)?;
            let (val, reactive) = query.get().expect("Unsatisfied components");
            reactive.update(val, state);
            Ok(())
        })
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
        let hovered_widget = intersect_widget(world, cursor_pos);

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
                    state.set_focus(hovered_widget, events);
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
                state.unfocus(events);
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
                    state.unfocus(events);
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
fn intersect_widget(world: &World, point: Position2D) -> Option<FocusedWidget> {
    world
        .query::<(&Position2D, &Size2D, &WidgetDepth)>()
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
        .map(|(e, _)| FocusedWidget::new(e, world.get::<Sticky>(e).ok().is_some()))
}

fn box_intersection(pos: Position2D, size: Size2D, point: Vec2) -> bool {
    let pos = *pos;
    let size = *size;

    point.x > pos.x - size.x
        && point.x < pos.x + size.x
        && point.y > pos.y - size.y
        && point.y < pos.y + size.y
}
