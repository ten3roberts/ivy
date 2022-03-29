use glam::Mat4;
use hecs::World;
use hecs_schedule::{CommandBuffer, Read, SubWorld, Write};
use ivy_base::{Position, Rotation, DEG_180};
use ivy_resources::{Handle, ResourceView};

use crate::{BoundingSphere, Camera, Mesh};

/// Updates the view matrix from camera [ `Position` ] and optional [ `Rotation` ]
pub fn update_view_matrices(world: SubWorld<(&mut Camera, &Position, &Rotation)>) {
    world
        .query::<(&mut Camera, &Position, Option<&Rotation>)>()
        .into_iter()
        .for_each(|(_, (camera, position, rotation))| {
            let view = match rotation {
                Some(rotation) => (Mat4::from_translation(**position)
                    * rotation.into_matrix()
                    * Mat4::from_rotation_y(DEG_180))
                .inverse(),

                None => Mat4::from_translation(-**position) * Mat4::from_rotation_y(DEG_180),
            };

            camera.set_view(view);
        })
}

pub fn add_bounds(
    world: SubWorld<&Handle<Mesh>>,
    resources: ResourceView<Mesh>,
    mut cmd: Write<CommandBuffer>,
) {
    world
        .query::<&Handle<Mesh>>()
        .without::<BoundingSphere>()
        .iter()
        .for_each(|(e, mesh)| cmd.insert_one(e, resources.get(*mesh).unwrap().bounds()))
}
