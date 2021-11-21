use crate::{
    bundles::*,
    collision::resolve_collision,
    components::*,
    connections::{Connection, ConnectionKind},
    Result,
};
use hecs::{Entity, World};
use hecs_hierarchy::Hierarchy;
use ivy_base::{Position, Rotation};
use ivy_collision::Collision;
use ultraviolet::{Bivec3, Rotor3};

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

/// Returns the root of the rigid system, along with its mass
pub fn get_rigid_root(world: &World, child: Entity) -> Result<(Entity, Mass)> {
    let mut system_mass = *world.get::<Mass>(child)?;
    let mut root = child;

    for val in world.ancestors::<Connection>(child) {
        root = val;
        system_mass += *world.get::<Mass>(val)?;
        dbg!(system_mass);

        match *world.get::<ConnectionKind>(child)? {
            ConnectionKind::Rigid => {}
            ConnectionKind::Spring {
                strength: _,
                dampening: _,
            } => break,
        };
    }

    Ok((root, system_mass))
}

pub fn resolve_collisions<I: Iterator<Item = Collision>>(
    world: &mut World,
    mut collisions: I,
) -> Result<()> {
    collisions.try_for_each(|coll| -> Result<()> {
        assert_ne!(coll.a, coll.b);

        // Trace up to the root of the rigid connection before solving
        // collisions
        let (a, a_mass) = get_rigid_root(world, coll.a)?;
        let (b, b_mass) = get_rigid_root(world, coll.b)?;

        let mut a_query = world.query_one::<(RbQuery, &Position)>(a)?;
        let (mut a, a_pos) = a_query.get().unwrap();
        let a_pos = *a_pos;

        let mut b_query = world.query_one::<(RbQuery, &Position)>(b)?;

        let (mut b, b_pos) = b_query.get().unwrap();
        let b_pos = *b_pos;

        // Modify mass to include all children masses
        a.mass = &a_mass;
        b.mass = &b_mass;
        let total_mass = a_mass + b_mass;

        let impulse = resolve_collision(&coll.contact, &a, a_pos, &b, b_pos);

        drop((a_query, b_query));

        {
            let dir = coll.contact.normal * coll.contact.depth;

            let mut pos = world.get_mut::<Position>(coll.a)?;
            *pos -= Position(dir * (*b_mass / *total_mass));
            drop(pos);

            let mut pos = world.get_mut::<Position>(coll.b)?;
            *pos += Position(dir * (*a_mass / *total_mass));
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
