use anyhow::Context;
use flax::{components::child_of, BoxedSystem, Dfs, DfsBorrow, Query, System, World};
use glam::{Mat4, Quat, Vec3};

use crate::{
    components::{position, rotation, scale, world_transform},
    AsyncCommandBuffer,
};

pub fn update_transform_system() -> BoxedSystem {
    System::builder()
        .with_query(
            // TODO: be smarter about this, sleeping entities etc
            Query::new((world_transform().as_mut(), position(), rotation(), scale()))
                .with_strategy(Dfs::new(child_of)),
        )
        .build(|mut query: DfsBorrow<_, _>| {
            query.traverse(
                &Mat4::IDENTITY,
                |(world_transform, &position, &rotation, &scale): (
                    &mut Mat4,
                    &Vec3,
                    &Quat,
                    &Vec3,
                ),
                 _,
                 parent| {
                    *world_transform =
                        *parent * Mat4::from_scale_rotation_translation(scale, rotation, position);
                    *world_transform
                },
            );
        })
        .boxed()
}

pub fn apply_async_commandbuffers(cmd: AsyncCommandBuffer) -> BoxedSystem {
    System::builder()
        .with_world_mut()
        .build(move |world: &mut World| {
            cmd.lock()
                .apply(world)
                .context("Failed to apply async commandbuffer")
        })
        .boxed()
}
