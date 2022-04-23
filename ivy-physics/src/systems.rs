use std::collections::BTreeMap;

use crate::{
    bundles::*,
    collision::{resolve_collision, ResolveObject},
    Effector, Result,
};
use glam::Quat;
use hecs::{Entity, Satisfies};
use hecs_hierarchy::{Hierarchy, HierarchyQuery};
use hecs_schedule::{traits::QueryExt, CommandBuffer, GenericWorld, Read, SubWorld, Write};
use ivy_base::{
    AngularVelocity, Connection, ConnectionKind, DeltaTime, Events, Friction, Gravity,
    GravityInfluence, Mass, Position, Resitution, Rotation, Sleeping, Static, Velocity,
};
use ivy_collision::{util::TOLERANCE, Collision, Contact};
use ivy_resources::{DefaultResource, DefaultResourceMut};

const BATCH_SIZE: u32 = 64;

pub fn integrate_velocity(
    world: SubWorld<(
        &mut Position,
        &mut Rotation,
        &AngularVelocity,
        &mut Velocity,
    )>,
    dt: Read<DeltaTime>,
    mut cmd: Write<CommandBuffer>,
) {
    world
        .native_query()
        .without::<Static>()
        .without::<Sleeping>()
        .iter()
        .for_each(|(e, (pos, rot, w, vel))| {
            *pos += Position(**vel * **dt);
            let mag = w.length();
            if mag > 0.2 {
                let w = Quat::from_axis_angle(w.0 / mag, mag * **dt);
                *rot = Rotation(w * rot.0);
            } else if vel.length_squared() < 0.01 {
                cmd.insert_one(e, Sleeping)
            }
        });
}

pub fn gravity(
    world: SubWorld<(&GravityInfluence, &Mass, &mut Effector)>,
    gravity: Read<Gravity>,
    collisions: DefaultResource<CollisionState>,
) {
    if gravity.length_squared() < TOLERANCE {
        return;
    }

    world
        .native_query()
        .without::<Static>()
        .without::<Sleeping>()
        .par_for_each(BATCH_SIZE, |(e, (influence, mass, effector))| {
            let supported = collisions.has_collision(e);
            effector.apply_force(**gravity * **influence * **mass, !supported)
        })
}

pub fn wrap_around_system(world: SubWorld<&mut Position>) {
    world.native_query().iter().for_each(|(_, pos)| {
        if pos.y < -100.0 {
            pos.y = 100.0
        }
    });
}

/// Returns the root of the rigid system, along with its mass
pub fn get_rigid_root(world: &impl GenericWorld, child: Entity) -> Result<(Entity, Mass)> {
    let mut system_mass = world
        .try_get::<Mass>(child)
        .ok()
        .as_deref()
        .cloned()
        .unwrap_or_default();

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

#[derive(Debug, Clone)]
pub struct CollisionState {
    sleeping: BTreeMap<(Entity, Entity), Collision>,
    active: BTreeMap<(Entity, Entity), Collision>,
}

impl CollisionState {
    pub fn new() -> Self {
        Self {
            active: BTreeMap::new(),
            sleeping: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, col: Collision) {
        let slot = if col.a.state.dormant() && col.b.state.dormant() {
            &mut self.sleeping
        } else {
            &mut self.active
        };

        slot.insert((col.a.entity, col.b.entity), col.clone());
        slot.insert((col.b.entity, col.a.entity), col.clone());
    }

    pub fn next_frame(&mut self, world: &impl GenericWorld) {
        let mut q = world.try_query::<hecs::Or<&Sleeping, &Static>>().unwrap();

        let q = q.view();
        self.active.clear();
        self.sleeping
            .retain(|_, v| q.get(v.a.entity).is_some() && q.get(v.b.entity).is_some());
    }

    pub fn has_collision(&self, e: Entity) -> bool {
        self.active
            .iter()
            .skip_while(move |((a, _), _)| *a != e)
            .next()
            .is_some()
    }

    pub fn get<'a>(&'a self, e: Entity) -> impl Iterator<Item = &'a Collision> {
        self.active
            .iter()
            .skip_while(move |((a, _), _)| *a != e)
            .take_while(move |((a, _), _)| *a == e)
            .chain(
                self.sleeping
                    .iter()
                    .skip_while(move |((a, _), _)| *a == e)
                    .take_while(move |((a, _), _)| *a == e),
            )
            .map(|(_, v)| v)
    }

    pub fn get_all(&self) -> impl Iterator<Item = (Entity, Entity, &Collision)> {
        self.active
            .iter()
            .chain(self.sleeping.iter())
            .map(|((a, b), v)| (*a, *b, v))
    }
}

impl Default for CollisionState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn resolve_collisions<I: Iterator<Item = Collision>>(
    world: SubWorld<(
        RbQuery,
        &Position,
        &mut Effector,
        HierarchyQuery<Connection>,
        &ConnectionKind,
        &Static,
        &Sleeping,
    )>,
    mut state: DefaultResourceMut<CollisionState>,
    mut collisions: I,
    dt: Read<DeltaTime>,
    _events: Read<Events>, // Wait for events
) -> Result<()> {
    state.next_frame(&world);
    collisions.try_for_each(|col| -> Result<()> {
        state.register(col.clone());

        // Ignore triggers
        if col.a.is_trigger || col.b.is_trigger {
            return Ok(());
        }
        // Check for static collision
        else if col.a.state.is_static() {
            return resolve_static(&world, col.a.entity, col.b.entity, col.contact, *dt);
        } else if col.b.state.is_static() {
            return resolve_static(
                &world,
                col.b.entity,
                col.a.entity,
                Contact {
                    points: col.contact.points.reverse(),
                    depth: col.contact.depth,
                    normal: -col.contact.normal,
                },
                *dt,
            );
        } else if col.a.state.is_static() && col.b.state.is_static() {
            return Ok(());
        }

        assert_ne!(col.a, col.b);

        // Trace up to the root of the rigid connection before solving
        // collisions
        let (a, a_mass) = get_rigid_root(&world, *col.a)?;
        let (b, b_mass) = get_rigid_root(&world, *col.b)?;

        // Ignore collisions between two immovable objects
        if !a_mass.is_normal() && !b_mass.is_normal() {
            return Ok(());
        }

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

        let impulse = resolve_collision(&col.contact, &a, &b);

        drop((a_query, b_query));

        let dir = col.contact.normal * col.contact.depth;

        let mut effector = world.get_mut::<Effector>(*col.a)?;
        effector.apply_impulse_at(impulse, col.contact.points[0] - a.pos, true);
        effector.translate(-dir * (*a.mass / *total_mass));
        drop(effector);

        let mut effector = world.get_mut::<Effector>(*col.b)?;
        effector.apply_impulse_at(-impulse, col.contact.points[1] - b.pos, true);
        effector.translate(dir * (*b.mass / *total_mass));

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

        if !b.mass.is_normal() {
            return Ok(());
        }

        let impulse = resolve_collision(&contact, &a, &b);

        effector.apply_impulse_at(-impulse, contact.points[1] - b.pos, false);
        // effector.apply_force_at(b_f, contact.points[1] - b.pos);

        effector.translate(contact.normal * contact.depth);
    }

    Ok(())
}

/// Applies effectors to their respective entities and clears the effects.
pub fn apply_effectors(
    world: SubWorld<(
        RbQueryMut,
        &mut Position,
        &mut Effector,
        Satisfies<&Sleeping>,
    )>,
    mut cmd: Write<CommandBuffer>,
    dt: Read<DeltaTime>,
) {
    world.native_query().without::<Static>().iter().for_each(
        |(e, (rb, pos, effector, sleeping))| {
            if !sleeping || effector.should_wake() {
                *rb.vel += effector.net_velocity_change(**dt);
                *pos += effector.translation();

                *rb.ang_vel += effector.net_angular_velocity_change(**dt);
            }

            effector.set_mass(*rb.mass);
            effector.set_ang_mass(*rb.ang_mass);

            if sleeping && effector.should_wake() {
                cmd.remove_one::<Sleeping>(e)
            }

            effector.clear()
        },
    )
}
