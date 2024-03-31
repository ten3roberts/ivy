use flax::{entity_ids, BoxedSystem, CommandBuffer, Query, System, World};
use ivy_base::transform;
use ivy_resources::ResourceView;

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

pub fn add_bounds(world: &World, resources: ResourceView<Mesh>, cmd: &mut CommandBuffer) {
    Query::new((entity_ids(), mesh()))
        .without(bounding_sphere())
        .borrow(world)
        .iter()
        .for_each(|(id, mesh)| {
            cmd.set(
                id,
                bounding_sphere(),
                resources.get(*mesh).unwrap().bounds(),
            );
        });
}
