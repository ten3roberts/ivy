use anyhow::Context;
use flax::{
    components::child_of,
    fetch::{entity_refs, EntityRefs},
    filter::{All, ChangeFilter},
    BoxedSystem, ComponentMut, Dfs, DfsBorrow, FetchExt, Query, QueryBorrow, System, World,
};
use glam::{Mat4, Vec3};

use crate::{
    components::{parent_transform, position, world_transform, TransformQuery},
    AsyncCommandBuffer,
};

pub fn update_root_transforms_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new(entity_refs()).with_filter(position().modified()))
        .with_query(
            Query::new((
                parent_transform().as_mut(),
                world_transform().as_mut(),
                TransformQuery::new(),
            ))
            .with_strategy(Dfs::new(child_of)),
        )
        .build(
            |mut query: QueryBorrow<EntityRefs, (All, ChangeFilter<Vec3>)>,
             mut children: DfsBorrow<
                '_,
                (ComponentMut<Mat4>, ComponentMut<Mat4>, TransformQuery),
            >| {
                for id in &mut query {
                    children.traverse_from(
                        id.id(),
                        &None,
                        |(parent_transform, world_transform, item), _, &parent| {
                            let parent = parent.unwrap_or(*parent_transform);
                            *parent_transform = parent;
                            *world_transform = parent
                                * Mat4::from_scale_rotation_translation(
                                    *item.scale,
                                    *item.rotation,
                                    *item.pos,
                                );

                            Some(*world_transform)
                        },
                    );
                }
            },
        )
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
