use std::collections::HashSet;

use crate::collision::resolve_collision;
use crate::components::*;
use crate::Result;
use hecs::World;
use ivy_collision::Collider;
use ivy_core::{Events, Position, Rotation, Scale};
use ultraviolet::Rotor3;

use crate::{
    collision::Collision,
    components::{AngularVelocity, TransformMatrix, Velocity},
};

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

pub fn collision_system(world: &World, events: &mut Events) -> Result<()> {
    let mut checked = HashSet::new();

    world
        .query::<(&Position, &Rotation, &Scale, &Collider)>()
        .iter()
        .try_for_each(|(e1, (pos, rot, scale, a))| -> Result<_> {
            let a_transform = TransformMatrix::new(*pos, *rot, *scale);
            world
                .query::<(&Position, &Rotation, &Scale, &Collider)>()
                .iter()
                .try_for_each(|(e2, (pos, rot, scale, b))| -> Result<_> {
                    if checked.get(&(e2, e1)).is_some() {
                        return Ok(());
                    }
                    let b_transform = TransformMatrix::new(*pos, *rot, *scale);
                    if e1 == e2 {
                        return Ok(());
                    }

                    let intersection = ivy_collision::intersect(&*a_transform, &*b_transform, a, b);

                    if let Some(intersection) = intersection {
                        checked.insert((e1, e2));
                        events.send(Collision {
                            a: e1,
                            b: e2,
                            intersection,
                        })
                    }

                    Ok(())
                })
        })
}

pub fn resolve_collisions_system<I: Iterator<Item = Collision>>(
    world: &mut World,
    mut collisions: I,
) -> Result<()> {
    collisions
        .next()
        .map(|collision| -> Result<()> {
            if collision.a == collision.b {
                return Ok(());
            };

            let mut a_query = world.query_one::<RbQuery>(collision.a)?;
            let a = a_query.get().unwrap();

            let mut b_query = world.query_one::<RbQuery>(collision.b)?;

            let b = b_query.get().unwrap();

            let impulse = resolve_collision(collision.intersection, a, b);

            world
                .get_mut::<Effector>(collision.a)?
                .apply_impulse(impulse);

            world
                .get_mut::<Effector>(collision.b)?
                .apply_impulse(-impulse);

            Ok(())
        })
        .unwrap_or(Ok(()))
}

/// Applies effectors to their respective entities and clears the effects.
pub fn apply_effectors_system(world: &World, dt: f32) {
    world
        .query::<(&mut Velocity, &Mass, &mut Effector)>()
        .iter()
        .for_each(|(_, (vel, mass, effector))| {
            *vel += effector.net_effect(*mass, dt);
            effector.clear()
        })
}

struct Satisfied;

pub fn satisfy_objects(world: &mut World) {
    let entities = world
        .query_mut::<Option<&Effector>>()
        .without::<Satisfied>()
        .into_iter()
        .map(|(e, effector)| (e, effector.cloned()))
        .collect::<Vec<_>>();

    entities.into_iter().for_each(|(e, effector)| {
        let _ = world.insert(e, (effector.unwrap_or_default(), Satisfied));
    })
}
