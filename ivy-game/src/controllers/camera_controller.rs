use std::any::type_name;

use flax::{
    component,
    filter::{All, WithRelation},
    system, Component, ComponentMut, FetchExt, Query, QueryBorrow,
};
use glam::{Mat4, Quat, Vec3};
use ivy_core::{
    components::{engine, position, request_capture_mouse, rotation, world_transform},
    math::{Axis2D, Axis3D},
    transforms::TransformUpdatePlugin,
    plugin::{Plugin, PluginContext},
    update_layer::ScheduleSetBuilder,
    Bundle,
};
use ivy_input::{
    Action, BindingExt, CursorMoveBinding,
};
use ivy_assets::{stored::DynamicStore, AssetCache, Resource};
use ivy_core::{
    components::{engine, position, request_capture_mouse, rotation, world_transform},
    math::{Axis2D, Axis3D},
    transforms::TransformUpdatePlugin,
    plugin::Plugin,
    Bundle,
};

        let rotate_action = Action::new().with_binding(
            CursorMoveBinding::new()
                .decompose(Axis2D::Y)
                .amplitude(-0.001)
                .compose(Axis3D::X),
        );

        entity
            .set(camera_controller(), controller);
    }
}

/// Moves the camera to the currently followed entity
pub struct CameraTrackingPlugin;

impl Plugin for CameraTrackingPlugin {
    fn install(&self, ctx: PluginContext) -> anyhow::Result<()> {
        let PluginContext { world, assets, store, schedules } = ctx;
        schedules
            .per_tick_mut()
            .with_system(CameraController::update_system());

        Ok(())
    }

    fn after(&self) -> Vec<&str> {
        vec![type_name::<TransformUpdatePlugin>()]
    }
}
