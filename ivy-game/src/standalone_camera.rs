use flax::{Entity, World};
use glam::{vec3, Quat, Vec3};
use ivy_assets::AssetCache;
use ivy_core::{
    components::{main_camera, TransformBundle},
    update_layer::{Plugin, ScheduleSetBuilder},
    Bundle, EntityBuilderExt,
};
use ivy_physics::{rapier3d::prelude::RigidBodyType, RigidBodyBundle};
use ivy_wgpu::components::{environment_data, projection_matrix};

pub struct StandaloneCameraPlugin;

impl Plugin for StandaloneCameraPlugin {
    fn install(
        &self,
        world: &mut World,
        _: &AssetCache,
        _: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        Entity::builder().mount(StandaloneCameraBundle).spawn(world);

        Ok(())
    }
}

struct StandaloneCameraBundle;

impl Bundle for StandaloneCameraBundle {
    fn mount(self, entity: &mut flax::EntityBuilder) {
        entity
            .mount(TransformBundle::new(
                vec3(0.0, 10.0, 10.0),
                Quat::IDENTITY,
                Vec3::ONE,
            ))
            .mount(RigidBodyBundle::new(RigidBodyType::Dynamic).with_can_sleep(false))
            .set(main_camera(), ())
            .set_default(projection_matrix())
            .set_default(environment_data());
    }
}
