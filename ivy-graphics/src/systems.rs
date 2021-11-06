use hecs::World;
use ivy_base::{Position, Rotation, Scale};
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

struct Satisfied;

pub fn satisfy_objects(world: &mut World) {
    let entities = world
        .query_mut::<(Option<&Position>, Option<&Rotation>, Option<&Scale>)>()
        .without::<Satisfied>()
        .into_iter()
        .map(|(e, (p, r, s))| (e, p.cloned(), r.cloned(), s.cloned()))
        .collect::<Vec<_>>();

    entities.into_iter().for_each(|(e, p, r, s)| {
        let _ = world.insert(
            e,
            (
                p.unwrap_or_default(),
                r.unwrap_or_default(),
                s.unwrap_or_else(|| Scale::new(1.0, 1.0, 1.0)),
                Satisfied,
            ),
        );
    })
}
