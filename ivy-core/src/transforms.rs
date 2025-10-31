use flax::{
    components::child_of,
    fetch::{entity_refs, EntityRefs},
    filter::All,
    BoxedSystem, Component, ComponentMut, Dfs, DfsBorrow, Query, QueryBorrow, System,
};
use glam::{Mat4, Vec3};
use ivy_assets::stored::DynamicStore;

use crate::{
    components::{parent_transform, position, world_transform, TransformQuery},
    plugin::{Plugin, PluginContext},
};

pub struct TransformUpdatePlugin;

impl Plugin for TransformUpdatePlugin {
    fn install(&self, ctx: &mut PluginContext) -> anyhow::Result<()> {
        ctx.schedules
            .per_tick_mut()
            .with_system(update_root_transforms_system());

        Ok(())
    }
}

fn update_root_transforms_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new(entity_refs()).with_filter(position()))
        .with_query(
            Query::new((
                parent_transform().as_mut(),
                world_transform().as_mut(),
                TransformQuery::new(),
            ))
            .with_strategy(Dfs::new(child_of)),
        )
        .build(
            |mut query: QueryBorrow<EntityRefs, (All, Component<Vec3>)>,
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
