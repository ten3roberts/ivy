use std::{f32::consts::PI, future::ready};

use anyhow::Context;
use flax::{
    FetchExt, Query, QueryBorrow, World, component,
    components::{child_of, name},
    signal::{BoxedSignal, Signal},
};
use glam::Vec2;
use ivy_core::{
    Bundle,
    components::{engine, main_camera},
};

use ivy_input::{
    Action, CursorPositionBinding, InputState, KeyBinding, MouseButtonBinding,
    components::input_state,
    types::{Key, MouseButton, NamedKey},
};
use ivy_physics::{components::physics_state, rapier3d::prelude::QueryFilter};
use ivy_scene::camera::{self, CameraQuery};

use crate::plugin::{EditCommand, Selection, SetSelection, edit_commands, selection};

pub struct SelectTool {}

component! {
    select_tool: SelectTool,
    mouse_button_changed_signal: BoxedSignal<bool>,
    cursor_pos:Vec2,
    shift_input: bool,
}

pub struct SelectToolBundle {}

impl SelectToolBundle {
    pub fn new() -> Self {
        Self {}
    }
}

impl Default for SelectToolBundle {
    fn default() -> Self {
        Self {}
    }
}

impl Bundle for SelectToolBundle {
    fn mount(&self, entity: &mut flax::EntityBuilder) {
        let mouse_button_changed = Signal::builder("SelectTool::mouse_button")
            .with_query(Query::new((
                select_tool().as_mut(),
                cursor_pos(),
                shift_input(),
                (selection(), edit_commands())
                    .relation(child_of)
                    .expect_msg("EditCommands not found"),
            )))
            .with_query(Query::new(CameraQuery::new()).with(main_camera()))
            .with_world()
            .build(
                |id,
                 mut query: QueryBorrow<_>,
                 mut camera: QueryBorrow<CameraQuery, _>,
                 world: &World,
                 pressed: bool| {
                    let (tool, &cursor_pos, &shift_input, (selection, edit_commands)): (
                        &mut SelectTool,
                        &Vec2,
                        &bool,
                        (&Selection, &flume::Sender<EditCommand>),
                    ) = query.get(id)?;
                    let camera = camera.first().context("No main camera")?;

                    if !pressed {
                        return Ok(());
                    }
                    let ray = camera::screen_to_world_ray(cursor_pos, camera);

                    let physics = world.get(engine(), physics_state())?;

                    let hit = physics.cast_ray(ray, 1000.0, true, QueryFilter::exclude_fixed());

                    if let Some(hit) = hit {
                        let new_selection = if shift_input {
                            selection.append_toggle(hit.rigidbody_id)
                        } else {
                            Selection::single(hit.rigidbody_id)
                        };

                        edit_commands
                            .send(EditCommand::SetSelection(SetSelection { new_selection }))?;
                    } else if !shift_input {
                        edit_commands.send(EditCommand::SetSelection(SetSelection {
                            new_selection: Selection::default(),
                        }))?;
                    }

                    anyhow::Ok(())
                },
            );

        let input = InputState::new()
            .with_action(
                shift_input(),
                Action::new().with_binding(KeyBinding::new(Key::Named(NamedKey::Shift))),
            )
            .with_action(
                cursor_pos(),
                Action::new().with_binding(CursorPositionBinding::new(true)),
            )
            .with_signal_action(
                Action::new().with_binding(MouseButtonBinding::new(MouseButton::Left)),
                mouse_button_changed_signal(),
            );

        entity
            .set(name(), "SelectTool".into())
            .set(select_tool(), SelectTool {})
            .set(input_state(), input)
            .set(mouse_button_changed_signal(), mouse_button_changed);
    }
}
