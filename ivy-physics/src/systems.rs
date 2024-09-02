use core::f32;
use std::collections::BTreeMap;

use crate::{bundles::*, components::effector, response::resolve_collisions};
use flax::{
    BoxedSystem, Component, Entity, EntityRef, FetchExt, Query, QueryBorrow, System, World,
};
use glam::{vec3, Mat4, Quat, Vec3};
use ivy_collision::{components::collision_tree, Collision, CollisionTree};
use ivy_core::{
    components::{
        angular_velocity, connection, engine, gravity, gravity_influence, position, rotation,
        sleeping, velocity, world_transform,
    },
    gizmos::{Line, DEFAULT_THICKNESS},
    Color, ColorExt,
};

pub fn integrate_velocity_system(dt: f32) -> BoxedSystem {
    System::builder()
        .with_query(Query::new((position().as_mut(), velocity())).without(sleeping()))
        .for_each(move |(pos, vel)| {
            *pos += *vel * dt;
        })
        .boxed()
}

pub fn integrate_angular_velocity_system(dt: f32) -> BoxedSystem {
    System::builder()
        .with_query(Query::new((rotation().as_mut(), angular_velocity())).without(sleeping()))
        .for_each(move |(rot, &w)| {
            *rot = Quat::from_axis_angle(w.normalize_or_zero(), w.length() * dt) * *rot
        })
        .boxed()
}

pub fn gravity_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            gravity().source(engine()),
            effector().as_mut(),
            gravity_influence(),
        )))
        .for_each(|(&state, effector, &gravity_influence)| {
            effector.apply_acceleration(gravity_influence * state, true);
        })
        .boxed()
}

pub fn get_rigid_root<'a>(entity: &EntityRef<'a>) -> EntityRef<'a> {
    let mut entity = *entity;
    loop {
        if let Some((parent, _)) = entity.relations(connection).next() {
            entity = entity.world().entity(parent).unwrap();
        } else {
            return entity;
        }
    }
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
        self.active.keys().any(|v| v.0 == e)
    }

    pub fn get(&self, e: Entity) -> impl Iterator<Item = &'_ Collision> {
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

// fn clear_sleeping() -> BoxedSystem {
//     System::builder()
//         .with_query(Query::new(collision_state().as_mut()))
//         .for_each(|v| v.next_frame())
//         .boxed()
// }

impl Default for CollisionState {
    fn default() -> Self {
        Self::new()
    }
}

/// Resolves all pending collisions to be processed
pub fn resolve_collisions_system(dt: f32) -> BoxedSystem {
    System::builder()
        .with_world()
        .with_query(Query::new(collision_tree()))
        .build(
            move |world: &World, mut query: QueryBorrow<Component<CollisionTree>>| {
                query.for_each(|collision_tree| {
                    resolve_collisions(world, collision_tree, dt).unwrap();
                })
            },
        )
        .boxed()
}

pub fn gizmo_system(dt: f32) -> BoxedSystem {
    System::builder()
        .with_query(Query::new(ivy_core::components::gizmos()))
        .with_query(Query::new((
            world_transform(),
            velocity(),
            angular_velocity(),
            effector(),
        )))
        .build(
            move |mut gizmos: QueryBorrow<Component<ivy_core::gizmos::Gizmos>>,
                  mut query: QueryBorrow<(
                Component<Mat4>,
                Component<Vec3>,
                Component<Vec3>,
                Component<crate::Effector>,
            )>| {
                let mut gizmos = gizmos
                    .get(engine())?
                    .begin_section("effectors_gizmo_system");

                for (transform, &velocity, &w, effector) in query.iter() {
                    let origin = transform.transform_point3(Vec3::ZERO);

                    let dv = effector.net_velocity_change(dt);
                    gizmos.draw(Line::new(origin, dv, DEFAULT_THICKNESS, Color::red()));
                    gizmos.draw(Line::new(
                        origin,
                        transform.transform_vector3(Vec3::Z),
                        DEFAULT_THICKNESS,
                        Color::blue(),
                    ));
                    gizmos.draw(Line::new(
                        origin,
                        transform.transform_vector3(Vec3::X),
                        DEFAULT_THICKNESS,
                        Color::red(),
                    ));
                    gizmos.draw(Line::new(
                        origin,
                        transform.transform_vector3(Vec3::Y),
                        DEFAULT_THICKNESS,
                        Color::green(),
                    ));
                    gizmos.draw(Line::new(
                        origin,
                        velocity,
                        DEFAULT_THICKNESS,
                        Color::cyan(),
                    ));
                    gizmos.draw(Line::new(origin, w, DEFAULT_THICKNESS, Color::purple()));
                }

                anyhow::Ok(())
            },
        )
        .boxed()
}

/// Removes small unwanted floating point accumulation by cutting values toward zero
pub(crate) fn round_to_zero(v: Vec3) -> Vec3 {
    const THRESHOLD: f32 = 1e-4;

    vec3(
        if v.x.abs() < THRESHOLD { 0.0 } else { v.x },
        if v.y.abs() < THRESHOLD { 0.0 } else { v.y },
        if v.z.abs() < THRESHOLD { 0.0 } else { v.z },
    )
}

/// Applies effectors to their respective entities and clears the effects.
pub fn apply_effectors_system(dt: f32) -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            RbQueryMut::new(),
            position().as_mut(),
            effector().as_mut(),
            sleeping().satisfied(),
        )))
        .par_for_each(move |(rb, position, effector, is_sleeping)| {
            if !is_sleeping || effector.should_wake() {
                // tracing::info!(%physics_state.dt, ?effector, "updating effector");
                *rb.vel = round_to_zero(*rb.vel + effector.net_velocity_change(dt));
                *position = round_to_zero(*position + effector.translation());

                *rb.ang_vel = round_to_zero(*rb.ang_vel + effector.net_angular_velocity_change(dt));
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
