use hecs::World;
use ivy_base::{Position, Rotation};
use ultraviolet::Mat4;

use crate::Camera;

/// Updates the view matrix from camera [ `Position` ] and optional [ `Rotation` ]
pub fn update_view_matrices(world: &World) {
    world
        .query::<(&mut Camera, &Position, Option<&Rotation>)>()
        .into_iter()
        .for_each(|(_, (camera, position, rotation))| {
            let view = match rotation {
                Some(rotation) => (Mat4::from_translation(**position)
                    * rotation.into_matrix().into_homogeneous())
                .inversed(),

                None => Mat4::from_translation(-**position),
            };

            camera.set_view(view);
        })
}
