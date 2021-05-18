use hecs::World;
use ultraviolet::{Mat4, Rotor3};

use crate::components::{AngularVelocity, ModelMatrix, Position, Rotation, Scale};

pub fn generate_model_matrices(world: &mut World) {
    let without = world
        .query_mut::<(&Position,)>()
        .without::<ModelMatrix>()
        .into_iter()
        .map(|(e, _)| e)
        .collect::<Vec<_>>();

    without
        .iter()
        .for_each(|e| world.insert_one(*e, ModelMatrix(Mat4::identity())).unwrap());

    world
        .query_mut::<(&mut ModelMatrix, &Position, &Rotation, &Scale)>()
        .into_iter()
        .for_each(|(_, (model, pos, rot, scale))| {
            *model = ModelMatrix(
                Mat4::from_translation(**pos)
                    * Mat4::from_nonuniform_scale(**scale)
                    * rot.into_matrix().into_homogeneous(),
            );
        });

    world
        .query_mut::<(&mut ModelMatrix, &Position)>()
        .without::<Scale>()
        .without::<Rotation>()
        .into_iter()
        .for_each(|(_, (model, pos))| {
            *model = ModelMatrix(Mat4::from_translation(**pos));
        });

    world
        .query_mut::<(&mut ModelMatrix, &Position, &Rotation)>()
        .without::<Scale>()
        .into_iter()
        .for_each(|(_, (model, pos, rot))| {
            *model =
                ModelMatrix(Mat4::from_translation(**pos)) * rot.into_matrix().into_homogeneous();
        });
}

pub fn integrate_angular_velocity(world: &mut World, dt: f32) {
    world
        .query_mut::<(&mut Rotation, &AngularVelocity)>()
        .into_iter()
        .for_each(|(_, (rot, ang))| {
            let (x, y, z) = (ang.x, ang.y, ang.z);
            *rot = Rotation(**rot * Rotor3::from_euler_angles(x * dt, y * dt, z * dt));
        });
}
