use hecs::World;
use ivy_core::{Position, Rotation};
use ultraviolet::Rotor3;

use crate::components::{AngularVelocity, Velocity};

pub fn integrate_velocity_system(world: &World, dt: f32) {
    world
        .query::<(&mut Position, &Velocity)>()
        .iter()
        .for_each(|(_, (pos, vel))| *pos += Position(vel.0 * dt));
}

pub fn integrate_angular_velocity_system(world: &World, dt: f32) {
    world
        .query::<(&mut Rotation, &AngularVelocity)>()
        .into_iter()
        .for_each(|(_, (rot, ang))| {
            let (x, y, z) = (ang.x, ang.y, ang.z);
            *rot = Rotation(**rot * Rotor3::from_euler_angles(x * dt, y * dt, z * dt));
        });
}

// pub fn gravity_system(world: &World, dt: f32) {
//     world
//         .query::<&mut Velocity>()
//         .iter()
//         .for_each(|(_, vel)| vel.y -= 1.0 * dt)
// }

// pub fn wrap_around_system(world: &World) {
//     world.query::<&mut Position>().iter().for_each(|(_, pos)| {
//         if pos.y < -100.0 {
//             pos.y = 100.0
//         }
//     });
// }
