use std::f32::consts::PI;

use anyhow::Context;
use flax::{
    CommandBuffer, Component, ComponentMut, EntityRef, Query, QueryBorrow, World, component,
    components::name,
    filter::{All, With},
    signal::{BoxedSignal, DynSignal, Signal},
    system,
};
use glam::{Quat, Vec2};
use ivy_core::{
    Bundle,
    components::{engine, gizmos, main_camera},
    gizmos::Gizmos,
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

pub struct TransformTool {
    manipulator: TransformManipulator,
}

component! {
    transform_tool: TransformTool,
    pub settings: TransformSettings,
    mouse_button_changed: BoxedSignal<bool>,
    mouse_moved: BoxedSignal<Vec2>,

    cursor_pos:Vec2,
    shift_input: bool,
}

impl TransformTool {
    #[system(with_world, with_query(Query::new(CameraQuery::new()).with(main_camera())))]
    pub fn update_system(
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
    fn mount(self, entity: &mut flax::EntityBuilder) {
        let mouse_button_changed = Signal::builder("TransformTool::mouse_button")
            .with_query(Query::new((
                transform_tool().as_mut(),
                cursor_pos(),
                shift_input(),
            )))
            .with_query(Query::new(CameraQuery::new()).with(main_camera()))
            .with_world()
            .build(
                |id,
                 mut query: QueryBorrow<(
                    ComponentMut<TransformTool>,
                    Component<Vec2>,
                    Component<bool>,
                )>,
                 mut camera: QueryBorrow<CameraQuery, _>,
                 world: &World,
                 pressed| {
                    let (tool, &cursor_pos, &shift_input) = query.get(id)?;
                    let camera = camera.first().context("No main camera")?;

                    if pressed {
                        let ray = camera::screen_to_world_ray(cursor_pos, camera);

                        if tool.manipulator.try_start_move(ray, world) {
                            return Ok(());
                        }

                        let physics = world.get(engine(), physics_state())?;

                        let hit = physics.cast_ray(ray, 100.0, true, QueryFilter::new());

                        if !shift_input {
                            tool.manipulator.clear_entities();
                        }

                        if let Some(hit) = hit {
                            tool.manipulator
                                .toggle_entity(ManipulatedEntity::from_entity(
                                    world.entity(hit.rigidbody_id)?,
                                ));
                        }
                    } else {
                        tool.manipulator.finish_move(world);
                    }

                    anyhow::Ok(())
                },
            )
            .boxed();

        let mouse_moved = Signal::builder("TransformTool::mouse_moved")
            .with_query(Query::new((transform_tool().as_mut(), cursor_pos())))
            .with_query(Query::new(CameraQuery::new()).with(main_camera()))
            .with_world()
            .build(
                |id,
                 mut query: QueryBorrow<_>,
                 mut camera: QueryBorrow<CameraQuery, _>,
                 world: &World,
                 pos| {
                    let (tool, &cursor_pos): (&mut TransformTool, &Vec2) = query.get(id)?;

                    let camera = camera.first().context("No main camera")?;

                    let ray = camera::screen_to_world_ray(pos, camera);

                    tool.manipulator.handle_mouse_move(ray, world)?;

                    anyhow::Ok(())
                },
            )
            .boxed();

        let input = InputState::new()
            .with_action(
                shift_input(),
                Action::new().with_binding(KeyBinding::new(Key::Named(NamedKey::Shift))),
            )
            .with_trigger_action(
                Action::new().with_binding(KeyBinding::new(Key::Character("g".into()))),
                |entity: &EntityRef, _: &mut CommandBuffer, pressed: bool| {
                    if !pressed {
                        return Ok(());
                    }

                    let mut settings = entity.get_mut(settings())?;
                    let space = match settings.space {
                        ManipulationSpace::Global => ManipulationSpace::Local,
                        ManipulationSpace::Local => ManipulationSpace::View,
                        ManipulationSpace::View => ManipulationSpace::Global,
                    };

                    settings.space = space;
                    Ok(())
                },
            )
            .with_trigger_action(
                Action::new().with_binding(KeyBinding::new(Key::Character("l".into()))),
                |entity: &EntityRef, _: &mut CommandBuffer, pressed: bool| {
                    if !pressed {
                        return Ok(());
                    }

                    let mut settings = entity.get_mut(settings())?;

                    tracing::info!(current_snap_mode = ?settings.snap_mode);

                    if settings.snap_mode.is_absolute() {
                        settings.snap_mode = SnapMode::None;
                        settings.angle_snap = 0.0;
                    } else {
                        settings.snap_mode = SnapMode::Absolute(0.5);
                        settings.angle_snap = PI / 180.0 * 5.0;
                    }

                    tracing::info!("New Snap mode: {:?}", settings.snap_mode);

                    Ok(())
                },
            )
            .with_signal_action(
                Action::new().with_binding(MouseButtonBinding::new(MouseButton::Left)),
                self::mouse_button_changed(),
            )
            .with_action(
                cursor_pos(),
                Action::new().with_binding(CursorPositionBinding::new(true)),
            )
            .with_signal_action(
                Action::new().with_binding(CursorPositionBinding::new(true)),
                self::mouse_moved(),
            );

        entity
            .set(name(), "TransformTool".into())
            .set(
                transform_tool(),
                TransformTool {
                    manipulator: TransformManipulator::new(vec![]),
                },
            )
            .set(input_state(), input)
            .set(self::mouse_moved(), mouse_moved)
            .set(self::mouse_button_changed(), mouse_button_changed)
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
            .with_system(draw_system());

        Ok(())
    }
}
