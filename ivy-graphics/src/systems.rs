use flax::{entity_ids, BoxedSystem, Component, EntityIds, Query, QueryBorrow, System, World};
use ivy_base::transform;
use ivy_resources::{Handle, ResourceView, Resources};

use crate::{
    components::{bounding_sphere, camera, mesh},
    Mesh,
};

/// Updates the view matrix from camera [ `Position` ] and optional [ `Rotation` ]
pub fn update_view_matrices() -> BoxedSystem {
    System::builder()
        .with_query(Query::new((camera().as_mut(), transform())))
        .for_each(|(camera, transform)| {
            // let view = (Mat4::from_translation(**position)
            //     * rotation.into_matrix()
            //     * Mat4::from_rotation_y(DEG_180));

            camera.set_view(transform.inverse());
        })
        .boxed()
}

pub fn add_bounds_system() -> BoxedSystem {
    System::builder()
        .with_cmd_mut()
        .with_input::<Resources>()
        .with_query(Query::new((entity_ids(), mesh())).without(bounding_sphere()))
        .build(
            |cmd: &mut flax::CommandBuffer,
             resources: &Resources,
             mut query: QueryBorrow<(EntityIds, Component<Handle<Mesh>>), _>| {
                query.iter().for_each(|(id, mesh)| {
                    cmd.set(
                        id,
                        bounding_sphere(),
                        resources.get(*mesh).unwrap().bounds(),
                    );
                });
            },
        )
        .boxed()
}
