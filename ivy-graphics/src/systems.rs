use glam::Mat4;
use hecs::World;
use ivy_base::{Position, Rotation, DEG_180};

use crate::Camera;

/// Updates the view matrix from camera [ `Position` ] and optional [ `Rotation` ]
pub fn update_view_matrices(world: &World) {
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
