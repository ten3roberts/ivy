use core::f32;
use std::collections::BTreeMap;

use crate::{
    bundles::*,
    components::{effector, resolver},
    response::Resolver,
};
use flax::{
    fetch::Source, BoxedSystem, Component, Entity, EntityRef, FetchExt, Query, QueryBorrow, System,
    World,
};
use glam::{vec3, Mat4, Quat, Vec3};
use ivy_collision::{components::collision_tree, CollisionTree, Contact};
use ivy_core::{
    components::{
        angular_velocity, connection, engine, gizmos, gravity, gravity_influence, position,
        rotation, sleeping, velocity, world_transform,
    },
    gizmos::{Gizmos, Line, DEFAULT_RADIUS, DEFAULT_THICKNESS},
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
    sleeping: BTreeMap<(Entity, Entity), Contact>,
    active: BTreeMap<(Entity, Entity), Contact>,
}

impl CollisionState {
    pub fn new() -> Self {
        Self {
            active: BTreeMap::new(),
            sleeping: BTreeMap::new(),
        }
    }

    pub fn register(&mut self, col: Contact) {
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

    pub fn get(&self, e: Entity) -> impl Iterator<Item = &'_ Contact> {
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

    pub fn get_all(&self) -> impl Iterator<Item = (Entity, Entity, &Contact)> {
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
pub fn resolve_collisions_system() -> BoxedSystem {
    System::builder()
        .with_world()
        .with_query(Query::new((collision_tree(), resolver())))
        .build(
            move |world: &World, mut query: QueryBorrow<(Component<CollisionTree>, Component<Resolver>)>| {
                if let Some((tree, resolver)) = query.first() {
                    for (_, contact) in tree.contacts() {
                        // for (_, island) in tree.islands() {
                        //     for (_, contact) in tree.island_contacts(island) {
                        resolver.resolve_contact(world, contact)?;
                        // }
                    }
                }

                anyhow::Ok(())
            },
        )
        .boxed()
}

/// Resolves all pending collisions to be processed
pub fn contact_gizmos_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new(gizmos().source(engine())))
        .with_query(Query::new(collision_tree()))
        .build(
            move |mut gizmos: QueryBorrow<Source<Component<Gizmos>, Entity>>,
                  mut query: QueryBorrow<Component<CollisionTree>>| {
                let mut gizmos = gizmos
                    .first()
                    .unwrap()
                    .begin_section("contact_gizmos_system");

                if let Some(tree) = query.first() {
                    for (_, island) in tree.islands() {
                        for (_, contact) in tree.island_contacts(island) {
                            gizmos.draw(&contact.surface);
                        }
                    }
                }
                anyhow::Ok(())
            },
        )
        .boxed()
}
pub fn island_graph_gizmo_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new(gizmos().source(engine())))
        .with_query(Query::new(collision_tree()))
        .with_query(Query::new(world_transform()))
        .build(
            move |mut gizmos: QueryBorrow<Source<Component<Gizmos>, Entity>>,
                  mut query: QueryBorrow<Component<CollisionTree>>,
                  mut transforms: QueryBorrow<Component<Mat4>>| {
                let mut gizmos = gizmos
                    .first()
                    .unwrap()
                    .begin_section("island_graph_gizmo_system");

                if let Some(tree) = query.first() {
                    for (i, (_, island)) in tree.islands().enumerate() {
                        let color = Color::from_hsla(i as f32 * 25.0, 0.7, 0.5, 1.0);

                        for (_, contact) in tree.island_contacts(island) {
                            let a = tree.body(contact.a.body);
                            let b = tree.body(contact.b.body);

                            let a_transform = transforms.get(a.id).copied().unwrap();
                            let b_transform = transforms.get(b.id).copied().unwrap();

                            let a_pos = a_transform.transform_point3(Vec3::ZERO);
                            let b_pos = b_transform.transform_point3(Vec3::ZERO);

                            gizmos.draw(ivy_core::gizmos::Sphere::new(
                                a_pos,
                                DEFAULT_RADIUS,
                                color,
                            ));
                            gizmos.draw(ivy_core::gizmos::Sphere::new(
                                b_pos,
                                DEFAULT_RADIUS,
                                color,
                            ));

                            gizmos.draw(Line::from_points(a_pos, b_pos, DEFAULT_THICKNESS, color))
                        }
                    }
                }
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
            move |mut gizmos: QueryBorrow<Component<Gizmos>>,
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
pub(crate) fn round_to_zero(v: Vec3, threshold: f32) -> Vec3 {
    vec3(
        if v.x.abs() < threshold { 0.0 } else { v.x },
        if v.y.abs() < threshold { 0.0 } else { v.y },
        if v.z.abs() < threshold { 0.0 } else { v.z },
    )
}

pub fn dampening_system(dt: f32) -> BoxedSystem {
    System::builder()
        .with_query(Query::new((RbQueryMut::new(),)))
        .par_for_each(move |(rb,)| {
            const LINEAR_DAMPEN: f32 = 0.1;
            const ANGULAR_DAMPEN: f32 = 0.1;

            *rb.vel = round_to_zero(*rb.vel * (1.0 / (1.0 + dt * LINEAR_DAMPEN)), 1e-2);
            *rb.ang_vel = round_to_zero(*rb.ang_vel * (1.0 / (1.0 + dt * ANGULAR_DAMPEN)), 1e-2);
        })
        .boxed()
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
                *rb.vel += round_to_zero(effector.net_velocity_change(dt), 1e-2);
                *position += effector.translation();

                *rb.ang_vel =
                    round_to_zero(*rb.ang_vel + effector.net_angular_velocity_change(dt), 1e-2);
            }

            effector.clear();
            effector.set_mass(*rb.mass);
            effector.set_ang_mass(*rb.ang_mass);
        })
        .boxed()
}
