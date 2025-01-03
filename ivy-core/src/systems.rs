use anyhow::Context;
use flax::{
    components::child_of, system, BoxedSystem, Dfs, DfsBorrow, FetchExt, Query, RelationExt,
    System, World,
};
use glam::{Mat4, Quat, Vec3};

use crate::{
    components::{position, rotation, scale, world_transform, TransformQuery},
    AsyncCommandBuffer,
};

// #[system(args(position=position(),rotation=rotation().modified(),scale=scale()), par)]
// pub fn update_root_transforms(
//     world_transform: &mut Mat4,
//     position: &Vec3,
//     rotation: &Quat,
//     scale: &Vec3,
// ) {
//     *world_transform = Mat4::from_scale_rotation_translation(*scale, *rotation, *position)
// }

pub fn update_root_transforms_system() -> BoxedSystem {
    System::builder()
        .with_query(
            Query::new((world_transform().as_mut(), TransformQuery::new().modified()))
                .batch_size(1024),
        )
        .par_for_each(|(world_transform, item)| {
            *world_transform =
                Mat4::from_scale_rotation_translation(*item.scale, *item.rotation, *item.pos)
        })
        .boxed()
}

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
