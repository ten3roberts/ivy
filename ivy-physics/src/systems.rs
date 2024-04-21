use std::collections::BTreeMap;

use crate::{
    bundles::*,
    collision::{resolve_collision, ResolveObject},
    components::{collision_state, effector, gravity_state, physics_state, PhysicsState},
    Effector, Result,
};
use flax::{
    BoxedSystem, Component, Entity, EntityRef, FetchExt, Mutable, Query, QueryBorrow, System, World,
};
use flume::Receiver;
use glam::Quat;
use ivy_base::{
    angular_velocity, connection, engine, friction, gravity_influence, position, restitution,
    rotation, sleeping, velocity,
};
use ivy_collision::{util::TOLERANCE, Collision, Contact};

const BATCH_SIZE: u32 = 64;

pub fn integrate_velocity() -> BoxedSystem {
    System::builder()
        .with_query(
            Query::new((
                physics_state().source(engine()),
                position().as_mut(),
                velocity(),
            ))
            .without(sleeping()),
        )
        .for_each(|(state, pos, vel)| {
            *pos += *vel * state.dt;
        })
        .boxed()
}

pub fn integrate_angular_velocity() -> BoxedSystem {
    System::builder()
        .with_query(
            Query::new((
                physics_state().source(engine()),
                rotation().as_mut(),
                angular_velocity(),
            ))
            .without(sleeping()),
        )
        .for_each(|(state, rot, &w)| {
            *rot *= Quat::from_axis_angle(w / w.length(), w.length() * state.dt);
        })
        .boxed()
}

// pub fn integrate_velocity(world: &World, dt: Read<DeltaTime>, mut cmd: Write<CommandBuffer>) {
//     world
//         .native_query()
//         .without::<Static>()
//         .without::<Sleeping>()
//         .iter()
//         .for_each(|(e, (pos, rot, w, vel, f))| {
//             *pos += Position(**vel * **dt);
//             let mag = w.length();
//             if mag > 0.2 {
//                 let w = Quat::from_axis_angle(w.0 / mag, mag * **dt);
//                 *rot = Rotation(w * rot.0);
//             } else if vel.length_squared() < 0.01 && !f.should_wake() {
//                 cmd.insert_one(e, Sleeping)
//             }
//         });
// }

pub fn gravity() -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            gravity_state(),
            effector().as_mut(),
            gravity_influence(),
        )))
        .for_each(|(state, effector, &gravity_influence)| {
            effector.apply_acceleration(gravity_influence * state.gravity, true);
        })
        .boxed()
}
// pub fn gravity(
//     world: SubWorld<(&GravityInfluence, &Mass, &mut Effector)>,
//     gravity: Read<Gravity>,
//     collisions: DefaultResource<CollisionState>,
// ) {
//     if gravity.length_squared() < TOLERANCE {
//         return;
//     }

//     world
//         .native_query()
//         .without::<Static>()
//         .without::<Sleeping>()
//         .par_for_each(BATCH_SIZE, |(e, (influence, mass, effector))| {
//             let supported = collisions.has_collision(e);
//             effector.apply_force(**gravity * **influence * **mass, !supported)
//         })
// }

// pub fn wrap_around_system(world: SubWorld<&mut Position>) {
//     world.native_query().iter().for_each(|(_, pos)| {
//         if pos.y < -100.0 {
//             pos.y = 100.0
//         }
//     });
// }

pub fn get_rigid_root<'a>(entity: &EntityRef<'a>) -> EntityRef<'a> {
    let mut current = *entity;
    loop {
        if let Some((parent, _)) = entity.relations(connection).next() {
            current = entity.world().entity(parent).unwrap();
        } else {
            return current;
        }
    }
}

#[derive(Debug, Clone)]
pub struct CollisionState {
    sleeping: BTreeMap<(Entity, Entity), Collision>,
    active: BTreeMap<(Entity, Entity), Collision>,
    pending: Vec<Collision>,
}

impl CollisionState {
    pub fn new() -> Self {
        Self {
            active: BTreeMap::new(),
            sleeping: BTreeMap::new(),
            pending: Vec::new(),
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

    pub fn next_frame(&mut self) {
        // todo!()
        // let mut q = world.try_query::<hecs::Or<&Sleeping, &Static>>().unwrap();
        // let query = Query::new()

        // let q = q.view();
        self.active.clear();
        // self.sleeping
        //     .retain(|_, v| q.get(v.a.entity).is_some() && q.get(v.b.entity).is_some());
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

fn clear_sleeping() -> BoxedSystem {
    System::builder()
        .with_query(Query::new(collision_state().as_mut()))
        .for_each(|v| v.next_frame())
        .boxed()
}

impl Default for CollisionState {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolves all pending collisions to be processed
pub fn resolve_collisions_system(collisions: Receiver<Collision>) -> BoxedSystem {
    System::builder()
        .with_world()
        .with_query(Query::new((collision_state().as_mut(), physics_state())))
        .build(
            move |world:&World, mut query: QueryBorrow<(Mutable<CollisionState>, Component<PhysicsState>)>| {
                query.for_each(|(collision_state, physics_state)| {
                    resolve_collisions(world, collision_state, collisions.try_iter(), physics_state.dt).unwrap();
                })
            },
        )
        .boxed()
}

pub fn resolve_collisions(
    world: &World,
    state: &mut CollisionState,
    collisions: impl Iterator<Item = Collision>,
    dt: f32,
) -> Result<()> {
    state.next_frame();

    for col in collisions {
        state.register(col.clone());

        let a = world.entity(col.a.entity)?;
        let b = world.entity(col.b.entity)?;

        // Ignore triggers
        if col.a.is_trigger || col.b.is_trigger {
            return Ok(());
        }
        // Check for static collision
        else if col.a.state.is_static() {
            resolve_static(&a, &b, col.contact, dt);
            continue;
        } else if col.b.state.is_static() {
            resolve_static(
                &b,
                &a,
                Contact {
                    points: col.contact.points.reverse(),
                    depth: col.contact.depth,
                    normal: -col.contact.normal,
                },
                dt,
            )?;
            continue;
        } else if col.a.state.is_static() && col.b.state.is_static() {
            continue;
        }

        assert_ne!(col.a, col.b);

        // Trace up to the root of the rigid connection before solving
        // collisions
        // let (a, a_mass) = get_rigid_root(&world.entity(*col.a))?;
        // let (b, b_mass) = get_rigid_root(&world.entity(*col.b)?)?;

        // let a_mass = world

        // // Ignore collisions between two immovable objects
        // if !a_mass.is_normal() && !b_mass.is_normal() {
        //     return Ok(());
        // }

        // let mut a_query = world.try_query_one::<(RbQuery, &Position, &Effector)>(a)?;
        // let (a, pos, eff) = a_query.get().unwrap();

        // // Modify mass to include all children masses

        // let a = ResolveObject {
        //     pos: *pos,
        //     vel: *a.vel + eff.net_velocity_change(**dt),
        //     ang_vel: *a.ang_vel,
        //     resitution: *a.resitution,
        //     mass: a.mass,
        //     ang_mass: *a.ang_mass,
        //     friction: *a.friction,
        // };

        // let mut b_query = world.try_query_one::<(RbQuery, &Position, &Effector)>(b)?;

        // let (b, pos, eff) = b_query.get().unwrap();

        // let b = ResolveObject {
        //     pos: *pos,
        //     vel: *b.vel + eff.net_velocity_change(**dt),
        //     ang_vel: *b.ang_vel,
        //     resitution: *b.resitution,
        //     mass: b_mass,
        //     ang_mass: *b.ang_mass,
        //     friction: *b.friction,
        // };

        // let total_mass = a.mass + b.mass;

        // let impulse = resolve_collision(&col.contact, &a, &b);

        // drop((a_query, b_query));

        // let dir = col.contact.normal * col.contact.depth;

        // let mut effector = world.get_mut::<Effector>(*col.a)?;
        // effector.apply_impulse_at(impulse, col.contact.points[0] - a.pos, true);
        // effector.translate(-dir * (*a.mass / *total_mass));

        // drop(effector);

        // let mut effector = world.get_mut::<Effector>(*col.b)?;
        // effector.apply_impulse_at(-impulse, col.contact.points[1] - b.pos, true);
        // effector.translate(dir * (*b.mass / *total_mass));
    }

    Ok(())
}

// Resolves collision with a static entity
fn resolve_static(a: &EntityRef, b: &EntityRef, contact: Contact, dt: f32) -> Result<()> {
    let query = &(
        restitution().opt_or_default(),
        friction().opt_or_default(),
        position(),
    );

    let mut a = a.query(&query);
    let a = a.get().unwrap();

    let query = &(RbQuery::new(), position(), effector().as_mut());

    let mut b = b.query(query);
    let Some(b) = b.get() else { return Ok(()) };
    let b_effector = b.2;

    let b = ResolveObject {
        pos: *b.1,
        vel: *b.0.vel + b_effector.net_velocity_change(dt),
        ang_vel: *b.0.ang_vel,
        resitution: *b.0.restitution,
        mass: *b.0.mass,
        ang_mass: *b.0.ang_mass,
        friction: *b.0.friction,
    };

    let a = ResolveObject {
        pos: *a.2,
        resitution: *a.0,
        friction: *a.1,
        ..Default::default()
    };

    // let mut b_query = world.try_query_one::<(RbQuery, &Position, &mut Effector)>(b)?;

    if !b.mass.is_normal() {
        return Ok(());
    }

    let impulse = resolve_collision(&contact, &a, &b);

    b_effector.apply_impulse_at(-impulse, contact.points[1] - b.pos, false);
    // effector.apply_force_at(b_f, contact.points[1] - b.pos);

    b_effector.translate(contact.normal * contact.depth);

    Ok(())
}

/// Applies effectors to their respective entities and clears the effects.
pub fn apply_effectors() -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            physics_state().source(engine()),
            RbQueryMut::new(),
            position().as_mut(),
            effector().as_mut(),
            sleeping().satisfied(),
        )))
        .for_each(|(physics_state, rb, position, effector, is_sleeping)| {
            if !is_sleeping || effector.should_wake() {
                // tracing::info!(%physics_state.dt, ?effector, "updating effector");
                *rb.vel += effector.net_velocity_change(physics_state.dt);
                *position += effector.translation();

                *rb.ang_vel += effector.net_angular_velocity_change(physics_state.dt);
            }

            effector.set_mass(*rb.mass);
            effector.set_ang_mass(*rb.ang_mass);

            // if sleeping && effector.should_wake() {
            //     cmd.remove_one::<Sleeping>(e)
            // }

            effector.clear()
        })
        .boxed()
}

// pub fn apply_effectors(
//     world: SubWorld<(
//         RbQueryMut,
//         &mut Position,
//         &mut Effector,
//         Satisfies<&Sleeping>,
//     )>,
//     mut cmd: Write<CommandBuffer>,
//     dt: Read<DeltaTime>,
// ) {
//     world.native_query().without::<Static>().iter().for_each(
//         |(e, (rb, pos, effector, sleeping))| {
//             if !sleeping || effector.should_wake() {
//                 *rb.vel += effector.net_velocity_change(**dt);
//                 *pos += effector.translation();

//                 *rb.ang_vel += effector.net_angular_velocity_change(**dt);
//             }

//             effector.set_mass(*rb.mass);
//             effector.set_ang_mass(*rb.ang_mass);

//             if sleeping && effector.should_wake() {
//                 cmd.remove_one::<Sleeping>(e)
//             }

//             effector.clear()
//         },
//     )
// }
