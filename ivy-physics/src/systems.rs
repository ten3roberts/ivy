use hecs::World;
use ivy_core::{Color, Position, Rotation, Scale};
use ultraviolet::Rotor3;

use crate::collision::Collider;
use crate::components::{AngularVelocity, TransformMatrix, Velocity};
use crate::gjk;

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
        .query::<(&Position, &Rotation, &Scale, &Collider, &mut Color)>()
        .iter()
        .for_each(|(e1, (pos, rot, scale, a, color))| {
            let a_transform = TransformMatrix::new(*pos, *rot, *scale);
            world
                .query::<(&Position, &Rotation, &Scale, &Collider)>()
                .iter()
                .for_each(|(e2, (pos, rot, scale, b))| {
                    let b_transform = TransformMatrix::new(*pos, *rot, *scale);
                    if e1 == e2 {
                        return;
                    }

                    let (intersect, simplex) = gjk::intersection(a_transform, b_transform, a, b);
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
