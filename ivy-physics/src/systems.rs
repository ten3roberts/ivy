use crate::collision::resolve_collision;
use crate::components::*;
use crate::Result;
use hecs::World;
use ivy_base::{Position, Rotation};
use ivy_collision::Collision;
use ultraviolet::Bivec3;
use ultraviolet::Rotor3;

use crate::components::{AngularVelocity, Velocity};

pub fn integrate_velocity(world: &World, dt: f32) {
    world
        .query::<(&mut Position, &Velocity)>()
        .iter()
        .for_each(|(_, (pos, vel))| *pos += Position(vel.0 * dt));
}

pub fn integrate_angular_velocity(world: &World, dt: f32) {
    world
        .query::<(&mut Rotation, &AngularVelocity)>()
        .into_iter()
        .for_each(|(_, (rot, w))| {
            let mag = w.mag();
            if mag > 0.0 {
                let w = Rotor3::from_angle_plane(mag * dt, Bivec3::from_normalized_axis(w.0 / mag));
                *rot = Rotation(w * rot.0);
            }
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

pub fn resolve_collisions<I: Iterator<Item = Collision>>(
    world: &mut World,
    mut collisions: I,
) -> Result<()> {
    collisions.try_for_each(|coll| -> Result<()> {
        assert_ne!(coll.a, coll.b);

        let mut a_query = world.query_one::<RbQuery>(coll.a)?;
        let a = a_query.get().unwrap();

        let mut b_query = world.query_one::<RbQuery>(coll.b)?;

        let b = b_query.get().unwrap();
        let a_pos = *a.pos;
        let b_pos = *b.pos;
        let a_mass = **a.mass;
        let b_mass = **b.mass;

        let total_mass = a_mass + b_mass;

        let impulse = resolve_collision(&coll.contact, &a, &b);

        drop((a_query, b_query));

        {
            let dir = coll.contact.normal * coll.contact.depth;

            let mut pos = world.get_mut::<Position>(coll.a)?;
            *pos -= Position(dir * (b_mass / total_mass));
            drop(pos);

            let mut pos = world.get_mut::<Position>(coll.b)?;
            *pos += Position(dir * (a_mass / total_mass));
            drop(pos);
        }

        let mut effector = world.get_mut::<Effector>(coll.a)?;
        effector.apply_impulse_at(impulse, coll.contact.points[0] - *a_pos);
        drop(effector);

        let mut effector = world.get_mut::<Effector>(coll.b)?;
        effector.apply_impulse_at(-impulse, coll.contact.points[1] - *b_pos);

        Ok(())
    })
}

/// Applies effectors to their respective entities and clears the effects.
pub fn apply_effectors(world: &World, dt: f32) {
    world
        .query::<(
            &mut Velocity,
            Option<&mut AngularVelocity>,
            &Mass,
            Option<&mut AngularMass>,
            &mut Effector,
        )>()
        .iter()
        .for_each(|(_, (vel, w, mass, angular_mass, effector))| {
            *vel += effector.net_velocity_change(*mass, dt);
            match (w, angular_mass) {
                (Some(w), Some(angular_mass)) => {
                    *w += effector.net_angular_velocity_change(*angular_mass, dt)
                }
                _ => {}
            }
            effector.clear()
        })
}

struct Satisfied;

pub fn satisfy_objects(world: &mut World) {
    let entities = world
        .query_mut::<(
            Option<&Effector>,
            Option<&Velocity>,
            Option<&AngularVelocity>,
            Option<&AngularMass>,
            Option<&Mass>,
            Option<&Resitution>,
        )>()
        .without::<Satisfied>()
        .into_iter()
        .map(|(e, (effector, vel, w, wm, m, res))| {
            (
                e,
                (
                    effector.cloned().unwrap_or_default(),
                    vel.cloned().unwrap_or_default(),
                    w.cloned().unwrap_or_default(),
                    wm.cloned().unwrap_or_default(),
                    m.cloned().unwrap_or_default(),
                    res.cloned().unwrap_or_default(),
                    Satisfied,
                ),
            )
        })
        .collect::<Vec<_>>();

    entities.into_iter().for_each(|(e, val)| {
        let _ = world.insert(e, val);
    })
}
