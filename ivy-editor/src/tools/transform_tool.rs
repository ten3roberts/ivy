use std::{f32::consts::PI, future::ready, sync::Arc};

use anyhow::Context;
use flax::{
    Component, Entity, FetchExt, Query, QueryBorrow, World, component,
    components::{child_of, name},
    filter::{All, With},
    signal::{BoxedSignal, Signal},
    system,
};
use futures::StreamExt;
use glam::{Quat, Vec2};
use ivy_assets::loadable::ResourceDesc;
use ivy_core::{
    Bundle,
    components::{engine, gizmos, main_camera},
    gizmos::Gizmos,
    palette::Srgba,
    template::BundleDesc,
    update_layer::Plugin,
};

use ivy_input::{
    Action, CursorPositionBinding, InputState, KeyBinding, MouseButtonBinding,
    components::input_state,
    types::{Key, MouseButton, NamedKey},
};
use ivy_physics::{components::physics_state, rapier3d::prelude::QueryFilter};
use ivy_scene::{
    camera::{self, CameraQuery},
    editor::manipulator::{
        ManipulatedEntity, ManipulationSpace, SnapMode, TransformManipulator, TransformSettings,
    },
};
use ivy_ui::{
    streamed::StreamedUiExt,
    violet::{
        core::{
            Scope, Widget,
            state::StateExt,
            style::{SizeExt, element_accent, spacing_medium, spacing_small},
            text::TextSegment,
            unit::Unit,
            widget::{
                Rectangle, Selectable, StreamWidget, Text, col, interactive::base::TooltipOptions,
                label, row,
            },
        },
        futures_signals::signal::{Mutable, SignalExt},
        lucide::icons::{LUCIDE_BOX, LUCIDE_GLOBE, LUCIDE_VIEW},
    },
};
use serde::{Deserialize, Serialize};

use crate::{
    plugin::{EditCommand, MoveEntities, Selection, SetSelection, edit_commands, selection},
    tools_controller::equip_signal,
};

pub struct TransformTool {
    manipulator: TransformManipulator,
}

component! {
    transform_tool: TransformTool,
    pub settings: TransformSettings,
    mouse_button_changed_signal: BoxedSignal<bool>,
    mouse_moved_signal: BoxedSignal<Vec2>,
    switch_modes_signal: BoxedSignal<bool>,
    switch_snap_mode_signal: BoxedSignal<bool>,
    cursor_pos:Vec2,
    shift_input: bool,
}

impl TransformTool {
    #[system(with_world, with_query(Query::new(CameraQuery::new()).with(main_camera())))]
    fn update_system(
        self: &mut TransformTool,
        settings: &TransformSettings,
        cursor_pos: Vec2,
        world: &World,
        camera_query: &mut QueryBorrow<CameraQuery, (All, With)>,
    ) {
        let camera = camera_query.first().expect("No camera found in the world");

        self.manipulator.set_settings(*settings);

        let ray = camera::screen_to_world_ray(cursor_pos, camera);
        self.manipulator
            .update(world, Quat::from_mat4(camera.transform), ray);
    }

    #[system(args(selection=selection().modified().relation(child_of)), with_world, filter(transform_tool().with()))]
    fn selection_changed_system(
        self: &mut TransformTool,
        selection: &Selection,
        world: &World,
    ) -> anyhow::Result<()> {
        tracing::info!("Selection changed: {:?}", selection);
        self.manipulator.clear_entities();
        for &entity in selection.entities().iter() {
            let entity = world.entity(entity)?;
            let manipulated = ManipulatedEntity::from_entity(entity);
            self.manipulator.add_entity(manipulated);
        }

        Ok(())
    }
}

#[system(with_query(Query::new(transform_tool())))]
pub fn draw_system(gizmos: &mut Gizmos, query: &mut QueryBorrow<Component<TransformTool>>) {
    let mut gizmos = gizmos.begin_section("TransformTool::draw_system");
    for transform_tool in query {
        transform_tool.manipulator.draw(&mut gizmos);
    }
}

pub struct TransformToolBundle {
    settings: TransformSettings,
}

impl TransformToolBundle {
    pub fn new(settings: TransformSettings) -> Self {
        Self { settings }
    }
}

impl Default for TransformToolBundle {
    fn default() -> Self {
        Self {
            settings: TransformSettings::default(),
        }
    }
}

impl Bundle for TransformToolBundle {
    fn mount(&self, entity: &mut flax::EntityBuilder) {
        let mouse_button_changed = Signal::builder("TransformTool::mouse_button")
            .with_query(Query::new((
                transform_tool().as_mut(),
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
                 pressed| {
                    let (tool, &cursor_pos, &shift_input, (selection, edit_commands)): (
                        &mut TransformTool,
                        &Vec2,
                        &bool,
                        (&Selection, &flume::Sender<EditCommand>),
                    ) = query.get(id)?;
                    let camera = camera.first().context("No main camera")?;

                    if pressed {
                        let ray = camera::screen_to_world_ray(cursor_pos, camera);

                        if tool.manipulator.try_start_move(ray, world) {
                            return Ok(());
                        }

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
                        } else if !selection.entities().is_empty() {
                            // If we clicked outside of any entity, clear the selection
                            edit_commands.send(EditCommand::SetSelection(SetSelection {
                                new_selection: Selection::default(),
                            }))?;
                        }
                    } else if let Some(cmd) = tool.manipulator.finish_move_cmd(world) {
                        if !cmd.is_empty() {
                            edit_commands
                                .send(EditCommand::MoveEntities(MoveEntities::new(cmd)))?;
                        }
                    }

                    anyhow::Ok(())
                },
            );

        let mouse_moved = Signal::builder("TransformTool::mouse_moved")
            .with_query(Query::new(transform_tool().as_mut()))
            .with_query(Query::new(CameraQuery::new()).with(main_camera()))
            .with_world()
            .build(
                |id,
                 mut query: QueryBorrow<_>,
                 mut camera: QueryBorrow<CameraQuery, _>,
                 world: &World,
                 pos| {
                    let tool: &mut TransformTool = query.get(id)?;

                    let camera = camera.first().context("No main camera")?;

                    let ray = camera::screen_to_world_ray(pos, camera);

                    tool.manipulator.handle_mouse_move(ray, world)?;

                    anyhow::Ok(())
                },
            );

        let switch_modes = Signal::builder("TransformTool::switch_modes")
            .with_query(Query::new(settings().as_mut()))
            .build(|id, mut query: QueryBorrow<_>, pressed: bool| {
                if !pressed {
                    return Ok(());
                }

                let settings: &mut TransformSettings = query.get(id)?;
                settings.space = match settings.space {
                    ManipulationSpace::Global => ManipulationSpace::Local,
                    ManipulationSpace::Local => ManipulationSpace::View,
                    ManipulationSpace::View => ManipulationSpace::Global,
                };

                anyhow::Ok(())
            });

        let snap_modes = Signal::builder("TransformTool::switch_snap_mode")
            .with_query(Query::new(settings().as_mut()))
            .build(|id, mut query: QueryBorrow<_>, pressed: bool| {
                if !pressed {
                    return Ok(());
                }

                let settings: &mut TransformSettings = query.get(id)?;

                if settings.snap_mode.is_absolute() {
                    settings.snap_mode = SnapMode::None;
                    settings.angle_snap = 0.0;
                } else {
                    settings.snap_mode = SnapMode::Absolute(0.5);
                    settings.angle_snap = PI / 180.0 * 5.0;
                }

                tracing::info!("New Snap mode: {:?}", settings.snap_mode);

                anyhow::Ok(())
            });

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
                Action::new().with_binding(KeyBinding::new(Key::Character("g".into()))),
                switch_modes_signal(),
            )
            .with_signal_action(
                Action::new().with_binding(KeyBinding::new(Key::Character("l".into()))),
                switch_snap_mode_signal(),
            )
            .with_signal_action(
                Action::new().with_binding(MouseButtonBinding::new(MouseButton::Left)),
                mouse_button_changed_signal(),
            )
            .with_signal_action(
                Action::new().with_binding(CursorPositionBinding::new(true)),
                mouse_moved_signal(),
            );

        let on_equip = Signal::builder("TransformTool::equip_signal")
            .with_world()
            .with(Query::new((
                transform_tool().as_mut(),
                selection().relation(child_of),
            )))
            .build(|id, world: &World, mut query: QueryBorrow<_>, ()| {
                let (tool, selection): (&mut TransformTool, &Selection) = query.get(id)?;

                for &entity in selection.entities().iter() {
                    let entity = world.entity(entity)?;
                    let manipulated = ManipulatedEntity::from_entity(entity);
                    tool.manipulator.add_entity(manipulated);
                }

                anyhow::Ok(())
            });

        entity
            .set(name(), "TransformTool".into())
            .set(
                transform_tool(),
                TransformTool {
                    manipulator: TransformManipulator::new(vec![]),
                },
            )
            .set(input_state(), input)
            .set(mouse_moved_signal(), mouse_moved)
            .set(mouse_button_changed_signal(), mouse_button_changed)
            .set(switch_modes_signal(), switch_modes)
            .set(switch_snap_mode_signal(), snap_modes)
            .set(equip_signal(), on_equip)
            .set(settings(), self.settings);
    }
}

pub struct TransformToolPlugin;

impl Plugin for TransformToolPlugin {
    fn install(
        &self,
        _: &mut World,
        _: &ivy_assets::AssetCache,
        _: &mut ivy_assets::stored::DynamicStore,
        schedules: &mut ivy_core::update_layer::ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        schedules
            .per_tick_mut()
            .with_system(TransformTool::update_system())
            .with_system(TransformTool::selection_changed_system())
            .with_system(draw_system());

        Ok(())
    }
}

pub struct TransformToolWidget {
    id: Entity,
}

impl TransformToolWidget {
    pub fn new(id: Entity) -> Self {
        Self { id }
    }
}

impl Widget for TransformToolWidget {
    fn mount(self, scope: &mut Scope<'_>) {
        let state = Mutable::new(None);

        scope.stream_component_duplex(settings(), self.id, state.clone());

        let space_state = Arc::new(
            state
                .clone()
                .lower_option()
                .memo(Default::default())
                .map_ref(|v| &v.space, |v| &mut v.space),
        );

        let settings = state
            .signal_ref(|s| s.clone())
            .to_stream()
            .filter_map(|v| ready(v))
            .map(move |v| {
                col((
                    row((
                        Selectable::new_value(
                            label(LUCIDE_GLOBE),
                            space_state.clone(),
                            ManipulationSpace::Global,
                        )
                        .with_tooltip(TooltipOptions::label("Align to World")),
                        Selectable::new_value(
                            label(LUCIDE_BOX),
                            space_state.clone(),
                            ManipulationSpace::Local,
                        )
                        .with_tooltip(TooltipOptions::label("Align to Object")),
                        Selectable::new_value(
                            label(LUCIDE_VIEW),
                            space_state.clone(),
                            ManipulationSpace::View,
                        )
                        .with_tooltip(TooltipOptions::label("Align to View")),
                    )),
                    label(format!(
                        "Snap: {}",
                        match v.snap_mode {
                            SnapMode::None => "None".to_string(),
                            SnapMode::Absolute(v) => v.to_string(),
                            SnapMode::Increment(v) => v.to_string(),
                        }
                    )),
                    label(format!("Angle Snap: {}°", v.angle_snap * 180.0 / PI)),
                    Rectangle::new(Srgba::new(0.2, 0.2, 0.2, 1.0))
                        .with_size(Unit::px2(0.0, 2.0))
                        .with_margin(spacing_medium()),
                    Text::formatted([
                        TextSegment::new("G").with_color(element_accent()),
                        TextSegment::new(" Change Space"),
                    ])
                    .with_margin(spacing_small()),
                    Text::formatted([
                        TextSegment::new("L").with_color(element_accent()),
                        TextSegment::new(" Toggle Snap"),
                    ])
                    .with_margin(spacing_small()),
                ))
                .with_stretch(true)
            });

        StreamWidget::new(settings).mount(scope);
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TransformToolBundleDesc {
    pub settings: TransformSettings,
}

impl ResourceDesc for TransformToolBundleDesc {
    type Output = TransformToolBundle;
    type Error = anyhow::Error;

    async fn load(&self, _assets: &ivy_assets::AssetCache) -> anyhow::Result<Self::Output> {
        Ok(TransformToolBundle {
            settings: self.settings.clone(),
        })
    }
}

#[typetag::serde]
impl BundleDesc for TransformToolBundleDesc {}
