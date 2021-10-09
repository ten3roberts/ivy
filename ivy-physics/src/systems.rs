use hecs::World;
use ivy_core::{Color, Position, Rotation};
use ultraviolet::Rotor3;

use crate::collision::Collider;
use crate::components::{AngularVelocity, Velocity};
use crate::gjk::{self};

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

pub fn gravity_system(world: &World, dt: f32) {
    world
        .query::<&mut Velocity>()
        .iter()
        .for_each(|(_, vel)| vel.y -= 1.0 * dt)
}

pub fn wrap_around_system(world: &World) {
    world.query::<&mut Position>().iter().for_each(|(_, pos)| {
        if pos.y < -100.0 {
            pos.y = 100.0
        }
    });
}

pub fn collision_system(world: &World) {
    world
        .query::<(&Position, &Collider, &mut Color)>()
        .iter()
        .for_each(|(e1, (a_pos, a, color))| {
            world
                .query::<(&Position, &Collider)>()
                .iter()
                .for_each(|(e2, (b_pos, b))| {
                    if e1 == e2 {
                        return;
                    }

                    let (intersect, simplex) = gjk::intersection(*a_pos, *b_pos, a, b);
                    // assert!(matches!(simplex, Simplex::Tetrahedron(a, b, c, d)));

                    if intersect {
                        eprintln!("{}: {:?}", intersect, simplex);
                        *color = Color::new(1.0, 0.0, 0.0, 1.0);
                    } else {
                        *color = Color::new(1.0, 1.0, 1.0, 1.0);
                    }
                })
        })
}
