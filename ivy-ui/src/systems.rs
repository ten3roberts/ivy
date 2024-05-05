use std::convert::TryInto;

use crate::{events::WidgetEvent, InteractiveState, Result};
use anyhow::Context;
use flax::{
    components::child_of, entity_ids, fetch::entity_refs, BoxedSystem, Entity, EntityRef, FetchExt,
    Query, System, World,
};
use glam::{Mat4, Vec2, Vec3Swizzles};
use glfw::Action;
use ivy_base::{position, size, visible, Events, Visible};
use ivy_graphics::components::camera;
use ivy_input::InputEvent;

use crate::{constraints::ConstraintQuery, *};

use self::constraints::calculate_relative;

/// Updates all UI trees and applies constraints.
/// Also updates canvas cameras.
pub fn update_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new((entity_refs(), canvas())).without_relation(child_of))
        .try_for_each(|(root, _)| {
            let world = root.world();
            apply_constraints(world, &root, Vec2::ZERO, Vec2::ONE, true)?;

            update_canvas(world, &root)?;

            update_from(world, &root, 1)?;

            anyhow::Ok(())
        })
        .boxed()
}

pub(crate) fn update_from(world: &World, entity: &EntityRef, depth: u32) -> Result<()> {
    let query = &(position(), size(), widget_depth().as_mut(), visible());

    let (position, size, is_visible) = {
        let mut query = entity.query(query);

        let (position, size, cur_depth, visible) = query.get().unwrap();

        // let mut query =
        //     world.try_query_one::<(&Position2D, &Size2D, &mut WidgetDepth, &mut Visible)>(parent)?;

        // let (position, size, curr_depth, visible) = query.get()?;
        *cur_depth = depth.into();
        (*position, *size, *visible)
    };

    let is_visible = is_visible.is_visible();

    if let Ok(layout) = entity.get(widget_layout()) {
        layout.update(world, entity, position.xy(), size, depth, is_visible)?;
    } else if let Ok(children) = entity.get(children()) {
        children.iter().try_for_each(|&child| {
            let entity = world.entity(child).unwrap();

            apply_constraints(world, &entity, position.xy(), size, is_visible)?;
            update_from(world, &entity, depth + 1)
        })?;
    }

    Ok(())
}

/// Applies the constraints associated to entity and uses the given parent.
pub(crate) fn apply_constraints(
    world: &World,
    entity: &EntityRef,
    parent_pos: Vec2,
    parent_size: Vec2,
    is_visible: bool,
) -> Result<()> {
    let query = &(
        ConstraintQuery::new(),
        position().as_mut(),
        size().as_mut(),
        visible().as_mut(),
    );

    let mut query = entity.query(query);

    // let mut constaints_query =
    //     world.try_query_one::<(ConstraintQuery, &mut Position2D, &mut Size2D, &mut Visible)>(
    //         entity,
    //     )?;
    let Some((constraints, pos, size, visible)) = query.get() else {
        return Ok(());
    };

    if !is_visible {
        *visible = Visible::HiddenInherit;
    } else if *visible == Visible::HiddenInherit {
        *visible = Visible::Visible;
    }

    *size = calculate_relative(*constraints.rel_size, parent_size) + *constraints.abs_size;

    *pos = (parent_pos
        + calculate_relative(*constraints.rel_offset, parent_size)
        + *constraints.abs_offset
        - *constraints.origin * *size)
        .extend(0.0);

    if *constraints.aspect != 0.0 {
        size.x = size.y * *constraints.aspect
    }

    Ok(())
}

/// Updates the canvas view and projection
pub fn update_canvas(world: &World, canvas: &EntityRef) -> Result<()> {
    let query = &(camera().as_mut(), size().as_mut(), position().as_mut());

    let mut query = canvas.query(query);
    let (camera, size, position) = query.get().unwrap();

    camera.set_orthographic(size.x * 2.0, size.y * 2.0, 0.0, 100.0);
    camera.set_view(Mat4::from_translation(-*position));

    Ok(())
}

pub fn reactive_system<T: 'static + Copy + Send + Sync, I: Iterator<Item = WidgetEvent>>(
    world: &World,
    events: I,
) -> Result<()> {
    Ok(())
}

pub fn handle_events(
    world: &mut World,
    mut events: &mut Events,
    state: &mut InteractiveState,
    cursor_pos: Vec2,
    intercepted_events: impl Iterator<Item = InputEvent>,
    control_events: impl Iterator<Item = UIControl>,
) {
    control_events.for_each(|event| match event {
        UIControl::Focus(widget) => state.set_focus(widget, true, &mut events),
    });

    let hovered = intersect_widget(&*world, cursor_pos);

    let sticky = hovered
        .map(|val| world.has(val, sticky()))
        .unwrap_or_default();

    for event in intercepted_events {
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

                    if let Ok(click) = entity.get(on_click()) {
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
            events.intercepted_send(event);
        }
    }
}

/// Returns the first widget that intersects the postiion
fn intersect_widget(world: &World, point: Vec2) -> Option<Entity> {
    Query::new((entity_ids(), position(), size(), widget_depth(), visible()))
        .with(interactive())
        .borrow(world)
        .iter()
        .filter_map(|(id, pos, size, depth, visible)| {
            if visible.is_visible() && box_intersection(pos.xy(), *size, point) {
                Some((id, depth))
            } else {
                None
            }
        })
        .max_by_key(|v| v.1)
        .map(|v| v.0)
}

fn box_intersection(pos: Vec2, size: Vec2, point: Vec2) -> bool {
    point.x > pos.x - size.x
        && point.x < pos.x + size.x
        && point.y > pos.y - size.y
        && point.y < pos.y + size.y
}
