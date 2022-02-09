use crate::{
    bundles::*,
    collision::{resolve_collision, ResolveObject},
    Effector, Result,
};
use glam::Quat;
use hecs::Entity;
use hecs_hierarchy::{Hierarchy, HierarchyQuery};
use hecs_schedule::{traits::QueryExt, GenericWorld, Read, SubWorld};
use ivy_base::{
    AngularVelocity, Connection, ConnectionKind, DeltaTime, Events, Friction, Gravity,
    GravityInfluence, Mass, Position, Resitution, Rotation, Static, Velocity,
};
use ivy_collision::{util::TOLERANCE, Collision, Contact};

const BATCH_SIZE: u32 = 64;

pub fn integrate_velocity(world: SubWorld<(&mut Position, &Velocity)>, dt: Read<DeltaTime>) {
    world
        .native_query()
        .without::<Static>()
        .par_for_each(BATCH_SIZE, |(_, (pos, vel))| *pos += Position(**vel * **dt));
}

pub fn gravity(world: SubWorld<(&GravityInfluence, &Mass, &mut Effector)>, gravity: Read<Gravity>) {
    if gravity.length_squared() < TOLERANCE {
        return;
    }

    world
        .native_query()
        .without::<Static>()
        .par_for_each(BATCH_SIZE, |(_, (influence, mass, effector))| {
            effector.apply_force(**gravity * **influence * **mass)
        })
}

pub fn integrate_angular_velocity(
    world: SubWorld<(&mut Rotation, &AngularVelocity)>,
    dt: Read<DeltaTime>,
) {
    world
        .native_query()
        .without::<Static>()
        .into_iter()
        .for_each(|(_, (rot, w))| {
            let mag = w.length();
            if mag > 0.0 {
                let w = Quat::from_axis_angle(w.0 / mag, mag * **dt);
                *rot = Rotation(w * rot.0);
            }
        });
}

// pub fn gravity_system(world: SubWorld<(&Velocity, &mut Effector)>) {
//     world
//         .native_query()
//         .without::<Static>()
//         .iter()
//         .for_each(|(_, ( vel , effector))| effector.apply_acceleration(dv))
// }

pub fn wrap_around_system(world: SubWorld<&mut Position>) {
    world.native_query().iter().for_each(|(_, pos)| {
        if pos.y < -100.0 {
            pos.y = 100.0
        }
    });
}

/// Returns the root of the rigid system, along with its mass
pub fn get_rigid_root(world: &impl GenericWorld, child: Entity) -> Result<(Entity, Mass)> {
    let mut system_mass = match world.try_get::<Mass>(child) {
        Ok(mass) => *mass,
        Err(_) => {
            panic!("No mass in leaf");
        }
    };

    let mut root = child;

    for val in world.ancestors::<Connection>(child) {
        root = val;
        system_mass += match world.try_get::<Mass>(val) {
            Ok(mass) => *mass,
            Err(_) => break,
        };

        match *world.try_get::<ConnectionKind>(child)? {
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
    world: SubWorld<(
        RbQuery,
        &Position,
        &mut Effector,
        HierarchyQuery<Connection>,
        &ConnectionKind,
    )>,
    mut collisions: I,
    dt: Read<DeltaTime>,
    _events: Read<Events>, // Wait for events
) -> Result<()> {
    collisions.try_for_each(|coll| -> Result<()> {
        // Ignore triggers
        if coll.a.is_trigger || coll.b.is_trigger {
            return Ok(());
        }
        // Check for static collision
        else if coll.a.is_static {
            return resolve_static(&world, coll.a.entity, coll.b.entity, coll.contact, *dt);
        } else if coll.b.is_static {
            return resolve_static(
                &world,
                coll.b.entity,
                coll.a.entity,
                Contact {
                    points: coll.contact.points.reverse(),
                    depth: coll.contact.depth,
                    normal: -coll.contact.normal,
                },
                *dt,
            );
        } else if coll.a.is_static && coll.b.is_static {
            return Ok(());
        }

        assert_ne!(coll.a, coll.b);

        // Trace up to the root of the rigid connection before solving
        // collisions
        let (a, a_mass) = get_rigid_root(&world, *coll.a)?;
        let (b, b_mass) = get_rigid_root(&world, *coll.b)?;

        let mut a_query = world.try_query_one::<(RbQuery, &Position, &Effector)>(a)?;
        let (a, pos, eff) = a_query.get().unwrap();

        // Modify mass to include all children masses

        let a = ResolveObject {
            pos: *pos,
            vel: *a.vel + eff.net_velocity_change(**dt),
            ang_vel: *a.ang_vel,
            resitution: *a.resitution,
            mass: a_mass,
            ang_mass: *a.ang_mass,
            friction: *a.friction,
        };

        let mut b_query = world.try_query_one::<(RbQuery, &Position, &Effector)>(b)?;

        let (b, pos, eff) = b_query.get().unwrap();

        let b = ResolveObject {
            pos: *pos,
            vel: *b.vel + eff.net_velocity_change(**dt),
            ang_vel: *b.ang_vel,
            resitution: *b.resitution,
            mass: b_mass,
            ang_mass: *b.ang_mass,
            friction: *b.friction,
        };

        let total_mass = a.mass + b.mass;

        let impulse = resolve_collision(&coll.contact, &a, &b);

        drop((a_query, b_query));

        let dir = coll.contact.normal * coll.contact.depth;

        let mut effector = world.get_mut::<Effector>(*coll.a)?;
        effector.apply_impulse_at(impulse, coll.contact.points[0] - a.pos);
        // effector.apply_force_at(a_f, coll.contact.points[0] - a.pos);
        effector.translate(-dir * (*b.mass / *total_mass));
        drop(effector);

        let mut effector = world.get_mut::<Effector>(*coll.b)?;
        effector.apply_impulse_at(-impulse, coll.contact.points[1] - b.pos);
        // effector.apply_force_at(b_f, coll.contact.points[1] - b.pos);
        effector.translate(dir * (*a.mass / *total_mass));

        Ok(())
    })
}

// Resolves a static collision
fn resolve_static(
    world: &impl GenericWorld,
    a: Entity,
    b: Entity,
    contact: Contact,
    dt: DeltaTime,
) -> Result<()> {
    let mut a_query =
        world.try_query_one::<(Option<&Resitution>, Option<&Friction>, &Position)>(a)?;
    let a = a_query
        .get()
        .expect("Static collider did not satisfy query");

    let a = ResolveObject {
        pos: *a.2,
        resitution: a.0.cloned().unwrap_or_default(),

        friction: a.1.cloned().unwrap_or_default(),
        ..Default::default()
    };

    let mut b_query = world.try_query_one::<(RbQuery, &Position, &mut Effector)>(b)?;

    if let Ok((rb, pos, effector)) = b_query.get() {
        let b = ResolveObject {
            pos: *pos,
            vel: *rb.vel + effector.net_velocity_change(*dt),
            ang_vel: *rb.ang_vel,
            resitution: *rb.resitution,
            mass: *rb.mass,
            ang_mass: *rb.ang_mass,
            friction: *rb.friction,
        };

        let impulse = resolve_collision(&contact, &a, &b);

        effector.apply_impulse_at(-impulse, contact.points[1] - b.pos);
        // effector.apply_force_at(b_f, contact.points[1] - b.pos);

        // effector.translate(contact.normal * contact.depth);
    }

    Ok(())
}

/// Applies effectors to their respective entities and clears the effects.
pub fn apply_effectors(
    world: SubWorld<(RbQueryMut, &mut Position, &mut Effector)>,
    dt: Read<DeltaTime>,
) {
    world
        .native_query()
        .without::<Static>()
        .iter()
        .for_each(|(_, (rb, pos, effector))| {
            *rb.vel += effector.net_velocity_change(**dt);
            *pos += effector.translation();

            *rb.ang_vel += effector.net_angular_velocity_change(**dt);

            effector.set_mass(*rb.mass);
            effector.set_ang_mass(*rb.ang_mass);

            effector.clear()
        })
}
