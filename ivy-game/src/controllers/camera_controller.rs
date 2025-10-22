use std::any::type_name;

use flax::{
    component,
    filter::{All, WithRelation},
    system, Component, ComponentMut, FetchExt, Query, QueryBorrow,
};
use glam::{Quat, Vec3};
use ivy_assets::{stored::DynamicStore, AssetCache, Resource};
use ivy_core::{
    components::{engine, position, request_capture_mouse, rotation, world_transform},
    math::{Axis2D, Axis3D},
    plugin::{Plugin, PluginContext},
    transforms::TransformUpdatePlugin,
    update_layer::ScheduleSetBuilder,
    Bundle,
};
use ivy_input::{Action, BindingExt, CursorMoveBinding};

use crate::controllers::character_controller::CharacterControllerPlugin;

component! {
    pub camera_target(id): (),
    pub camera_look_data: CameraLookData,
    camera_controller:CameraController,
}

/// Pure data determining where the camera should look
#[derive(Default)]
pub struct CameraLookData {
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
}

#[derive(Clone, Copy)]
pub enum CameraMode {
    FirstPerson,
    ThirdPerson,
}

pub struct CameraController {
    pub offset: Vec3,
    pub rotation_offset: Vec3,
    pub mode: CameraMode,
}

impl CameraController {
    pub fn new() -> Self {
        Self {
            offset: Vec3::ZERO,
            rotation_offset: Vec3::ZERO,
            mode: CameraMode::ThirdPerson,
        }
    }
}

impl CameraController {
    #[system(args(self=camera_controller().added()),with_query(Query::new(request_capture_mouse().as_mut())))]
    pub fn capture_mouse_system(
        self: &CameraController,
        query: &mut QueryBorrow<ComponentMut<bool>>,
    ) -> anyhow::Result<()> {
        *query.get(engine())? = true;
        Ok(())
    }

    #[system(args(self=camera_controller().as_mut(), rotation=rotation().as_mut(), position=position().as_mut()), with_query(Query::new((camera_look_data(), position(), rotation())).with_relation(camera_target)))]
    pub fn update_system(
        self: &mut CameraController,
        rotation: &mut Quat,
        position: &mut Vec3,
        target: &mut QueryBorrow<
            (Component<CameraLookData>, Component<Vec3>, Component<Quat>),
            (All, WithRelation),
        >,
    ) {
        if let Some((look_data, target_pos, target_rot)) = target.first() {
            match self.mode {
                CameraMode::FirstPerson => {
                    let yaw = Quat::from_rotation_y(look_data.yaw);
                    let pitch = Quat::from_rotation_x(1.0);

                    *rotation = target_rot * yaw * pitch;
                    *position = target_pos + target_rot * self.offset;
                }
                CameraMode::ThirdPerson => {
                    let azimuth = look_data.yaw;
                    let zenith = look_data.pitch;
                    let focus_point = target_pos + self.offset;
                    let dir =
                        target_rot * Quat::from_rotation_y(azimuth) * Quat::from_rotation_x(zenith);

                    *position = focus_point + (dir * Vec3::Z * 5.0);
                    *rotation = dir;
                }
            }
        }
    }
}

#[derive(Debug, Clone, Resource, Bundle)]
pub struct CameraControllerBundle {}

impl Bundle for CameraControllerBundle {
    fn mount(&self, entity: &mut flax::EntityBuilder) {
        let mut controller = CameraController::new();
        controller.offset = match controller.mode {
            CameraMode::FirstPerson => glam::vec3(0.0, 1.7, 0.0),
            CameraMode::ThirdPerson => glam::vec3(0.0, 1.7, 0.0),
        };

        entity.set(camera_controller(), controller);
    }
}

/// Moves the camera to the currently followed entity
pub struct CameraControllerPlugin;

impl Plugin for CameraControllerPlugin {
    fn install(&self, ctx: &mut PluginContext) -> anyhow::Result<()> {
        ctx.schedules
            .per_tick_mut()
            .with_system(CameraController::update_system());

        Ok(())
    }

    fn after(&self) -> Vec<&str> {
        vec![type_name::<CharacterControllerPlugin>()]
    }

    fn before(&self) -> Vec<&str> {
        vec![type_name::<TransformUpdatePlugin>()]
    }
}
