use flax::{Entity, World};
use glam::{vec3, Quat, Vec3};
use ivy_assets::{stored::DynamicStore, AssetCache};
use ivy_core::{
    components::{main_camera, TransformBundle},
    update_layer::{Plugin, ScheduleSetBuilder},
    Bundle, EntityBuilderExt,
};
use ivy_graphics::camera::CameraBundle;
use ivy_physics::{RigidBodyBundle, RigidBodyKind};

use crate::controllers::camera_controller::CameraControllerBundle;

pub struct StandaloneCameraPlugin;

impl Plugin for StandaloneCameraPlugin {
    fn install(
        &self,
        world: &mut World,
        _: &AssetCache,
        _: &mut DynamicStore,
        _: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        Entity::builder().mount(StandaloneCameraBundle).spawn(world);

        Ok(())
    }
}

struct StandaloneCameraBundle;

impl Bundle for StandaloneCameraBundle {
    fn mount(&self, entity: &mut flax::EntityBuilder) {
        entity
            .mount(TransformBundle::new(
                vec3(0.0, 10.0, 10.0),
                Quat::IDENTITY,
                Vec3::ONE,
            ))
            .mount(RigidBodyBundle::new(RigidBodyKind::Dynamic).with_can_sleep(false))
            .mount(CameraBundle::default())
            .mount(CameraControllerBundle {})
            .set(main_camera(), ());
    }
}
