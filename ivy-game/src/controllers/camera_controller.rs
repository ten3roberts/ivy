use std::any::type_name;

use flax::{
    component,
    filter::{All, With},
    system, Component, ComponentMut, Entity, FetchExt, Query, QueryBorrow,
};
use glam::{Mat4, Quat, Vec3};
use ivy::{
    engine,
    input::{components::input_state, Action, Axis2D, Axis3D, BindingExt, CursorMoveBinding},
    ivy_core::{
        transforms::TransformUpdatePlugin,
        update_layer::{Plugin, ScheduleSetBuilder},
        Bundle,
    },
    position, request_capture_mouse, rotation, world_transform, InputState,
};
use ivy_assets::{AssetCache, Resource};
use ivy_core::{components::world_transform, Bundle};

component! {
    pub camera_target(Entity): (),
    pub camera_look_data: CameraLookData,
    camera_controller:CameraController,
}

/// Pure data determining where the camera should look
pub struct CameraLookData {
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
}

pub struct CameraController {
    pub offset: Vec3,
    pub rotation_offset: Vec3,
}

impl CameraController {
    pub fn new() -> Self {
        Self {}
    }

    #[system(args(self=camera_controller().added()),with_query(Query::new(request_capture_mouse().as_mut())))]
    pub fn capture_mouse_system(
        self: &CameraController,
        query: &mut QueryBorrow<ComponentMut<bool>>,
    ) -> anyhow::Result<()> {
        *query.get(engine())? = true;
        Ok(())
    }

    #[system(with_query(Query::new((camera_look_data(), world_transform())).with_relation(camera_target)))]
    pub fn update_system(
        self: &mut CameraController,
        rotation: &mut Quat,
        position: &mut Vec3,
        target: &mut QueryBorrow<Component<Mat4>, (All, With)>,
    ) {
        if let Some((look_data, target_transform)) = target.first() {
            let (_, target_rot, target_pos) = target_transform.to_scale_rotation_translation();
            let yaw = Quat::from_rotation_y(look_data.yaw);
            let pitch = Quat::from_rotation_x(look_data.pitch);

            *rotation = yaw * pitch;
            *position = target_pos;
        } else {
            tracing::warn!("CameraController has no target to follow");
        }
    }
}

#[derive(Debug, Clone, Resource, Bundle)]
pub struct CameraControllerBundle {}

impl Bundle for CameraControllerBundle {
    fn mount(&self, entity: &mut flax::EntityBuilder) {
        let controller = CameraController::new();

        let rotate_action = Action::new().with_binding(
            CursorMoveBinding::new()
                .decompose(Axis2D::Y)
                .amplitude(-0.001)
                .compose(Axis3D::X),
        );

        entity
            .set(camera_controller(), controller)
            .set_default(camera_target());
    }
}

/// Moves the camera to the currently followed entity
pub struct CameraTrackingPlugin;

impl Plugin for CameraTrackingPlugin {
    fn install(
        &self,
        _: &mut flax::World,
        _: &AssetCache,
        schedules: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        schedules
            .per_tick_mut()
            .with_system(CameraController::update_system());

        Ok(())
    }

    fn after(&self) -> Vec<&str> {
        vec![type_name::<TransformUpdatePlugin>()]
    }
}
