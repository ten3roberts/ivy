use anyhow::Context;
use flax::{BoxedSystem, FetchExt, Query, System, World};
use glam::Mat4;

use crate::{position, rotation, scale, world_transform, AsyncCommandBuffer};

// TODO: child/parent

pub fn update_transform_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new(
            (world_transform().as_mut(), position(), rotation(), scale()).modified(),
        ))
        .par_for_each(|(world_transform, &position, &rotation, &scale)| {
            *world_transform = Mat4::from_scale_rotation_translation(scale, rotation, position);
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
