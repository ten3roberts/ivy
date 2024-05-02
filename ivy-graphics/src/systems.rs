use flax::{entity_ids, BoxedSystem, Component, EntityIds, Query, QueryBorrow, System, World};
use glam::Mat4;
use ivy_assets::Asset;
use ivy_base::{world_transform, DEG_180};

use crate::{
    components::{bounding_sphere, camera, mesh},
    Mesh,
};

/// Updates the view matrix from camera [ `Position` ] and optional [ `Rotation` ]
pub fn update_view_matrices() -> BoxedSystem {
    System::builder()
        .with_query(Query::new((camera().as_mut(), world_transform())))
        .for_each(|(camera, transform)| {
            // tracing::info!(
            //     transform = ?transform.to_scale_rotation_translation(),
            //     "updating transform"
            // );
            // let view = (Mat4::from_translation(**position)
            //     * rotation.into_matrix()
            //     * Mat4::from_rotation_y(DEG_180));

            camera.set_view((*transform * Mat4::from_rotation_y(DEG_180)).inverse());
        })
        .boxed()
}

pub fn add_bounds_system() -> BoxedSystem {
    System::builder()
        .with_cmd_mut()
        .with_query(Query::new((entity_ids(), mesh())).without(bounding_sphere()))
        .build(
            |cmd: &mut flax::CommandBuffer,
             mut query: QueryBorrow<(EntityIds, Component<Asset<Mesh>>), _>| {
                query.iter().for_each(|(id, mesh)| {
                    cmd.set(id, bounding_sphere(), mesh.bounds());
                });
            },
        )
        .boxed()
}
