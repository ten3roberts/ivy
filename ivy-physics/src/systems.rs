use core::f32;

use anyhow::Context;
use flax::{
    components::child_of,
    entity_ids,
    events::EventSubscriber,
    fetch::{Copied, Modified, TransformFetch},
    filter::ChangeFilter,
    BoxedSystem, CommandBuffer, Component, ComponentMut, EntityIds, FetchExt, Opt, Query,
    QueryBorrow, RelationExt, System, World,
};
use glam::{Mat4, Vec3};
use ivy_core::{
    components::{engine, main_camera, world_transform, TransformQuery, TransformQueryItem},
    gizmos::{Gizmos, Line, DEFAULT_THICKNESS},
    subscribers::{RemovedComponentSubscriber, RemovedRelationSubscriber},
    Color, ColorExt,
};
use rapier3d::{
    math::Isometry,
    prelude::{
        ColliderBuilder, ColliderHandle, LockedAxes, RigidBodyBuilder, RigidBodyHandle,
        RigidBodyType, SharedShape,
    },
};

use crate::{
    components::*,
    state::{
        BodyDynamicsQuery, BodyDynamicsQueryItem, BodyDynamicsQueryMut, ColliderDynamicsQuery,
        PhysicsState,
    },
};

#[allow(clippy::type_complexity)]
pub fn register_bodies_system() -> BoxedSystem {
    System::builder()
        .with_cmd_mut()
        .with_query(Query::new(physics_state().as_mut()))
        .with_query(Query::new((
            entity_ids(),
            rigid_body_type().modified(),
            locked_axes().opt(),
            can_sleep().satisfied(),
            gravity_influence().opt_or(1.0),
        )))
        .build(
            move |cmd: &mut CommandBuffer,
                  mut query: QueryBorrow<ComponentMut<PhysicsState>>,
                  mut bodies: QueryBorrow<
                '_,
                (
                    EntityIds,
                    ChangeFilter<RigidBodyType>,
                    Opt<Component<LockedAxes>>,
                    _,
                    _,
                ),
            >| {
                if let Some(state) = query.first() {
                    for (id, &body_type, locked_axes, can_sleep, &gravity) in bodies.iter() {
                        let rb = state.add_body(
                            id,
                            RigidBodyBuilder::new(body_type)
                                .can_sleep(can_sleep)
                                .locked_axes(locked_axes.copied().unwrap_or(LockedAxes::empty()))
                                .gravity_scale(gravity)
                                .build(),
                        );
                        cmd.set(id, rb_handle(), rb);
                    }
                }

                anyhow::Ok(())
            },
        )
        .boxed()
}

pub fn register_colliders_system() -> BoxedSystem {
    System::builder()
        .with_cmd_mut()
        .with_query(Query::new(physics_state().as_mut()))
        .with_query(Query::new((
            entity_ids(),
            (collider_shape(), density(), restitution(), friction()).added(),
            TransformQuery::new(),
            (entity_ids(), rb_handle()).traverse(child_of),
        )))
        .build(
            move |cmd: &mut CommandBuffer,
                  mut physics_state: QueryBorrow<ComponentMut<PhysicsState>>,
                  mut bodies: QueryBorrow<'_, _>| {
                if let Some(state) = physics_state.first() {
                    for (
                        id,
                        (shape, &density, &restitution, &friction),
                        transform,
                        (parent_id, &parent),
                    ) in bodies.iter()
                    {
                        let local_position = if parent_id == id {
                            Isometry::identity()
                        } else {
                            let transform: TransformQueryItem = transform;
                            Isometry::new(
                                (*transform.pos).into(),
                                transform.rotation.to_scaled_axis().into(),
                            )
                        };

                        let handle = state.attach_collider(
                            id,
                            ColliderBuilder::new(SharedShape::clone(shape))
                                .density(density)
                                .restitution(restitution)
                                .friction(friction)
                                .position(local_position)
                                .build(),
                            parent,
                        );

                        let rb = state.rigidbody(parent);
                        cmd.set(id, collider_handle(), handle)
                            .set(parent_id, mass(), rb.mass())
                            .set(parent_id, center_of_mass(), (*rb.center_of_mass()).into());
                    }
                }

                anyhow::Ok(())
            },
        )
        .boxed()
}

pub fn unregister_bodies_system(world: &mut World) -> BoxedSystem {
    let (tx, rx) = flume::unbounded();

    world.subscribe(RemovedComponentSubscriber::new(tx, rb_handle()));

    System::builder()
        .with_world()
        .with_cmd_mut()
        .with_query(Query::new(physics_state().as_mut()))
        .build(
            move |_: &World,
                  _: &mut CommandBuffer,
                  mut query: QueryBorrow<ComponentMut<PhysicsState>>| {
                if let Some(state) = query.first() {
                    for (_, rb_handle) in rx.try_iter() {
                        state.remove_body(rb_handle);
                    }
                }

                anyhow::Ok(())
            },
        )
        .boxed()
}

pub fn unregister_colliders_system(world: &mut World) -> BoxedSystem {
    let (tx, rx) = flume::unbounded();

    world.subscribe(RemovedComponentSubscriber::new(tx, collider_handle()));

    System::builder()
        .with_world()
        .with_cmd_mut()
        .with_query(Query::new(physics_state().as_mut()))
        .build(
            move |_: &World,
                  _: &mut CommandBuffer,
                  mut query: QueryBorrow<ComponentMut<PhysicsState>>| {
                if let Some(state) = query.first() {
                    for (_, handle) in rx.try_iter() {
                        state.remvoe_collider(handle);
                    }
                }

                anyhow::Ok(())
            },
        )
        .boxed()
}

pub fn attach_joints_system(world: &mut World) -> BoxedSystem {
    let (tx, rx) = flume::unbounded();

    world.subscribe(
        tx.filter_event_kind(flax::events::EventKindFilter::ADDED)
            .filter_relations([impulse_joint.as_relation().id()]),
    );

    let (removed_tx, removed_rx) = flume::unbounded();
    world.subscribe(RemovedRelationSubscriber::new(
        removed_tx,
        impulse_joint.as_relation(),
    ));

    System::builder()
        .with_world()
        .with_cmd_mut()
        .with_query(Query::new(physics_state().as_mut()))
        .build(
            move |world: &World,
                  cmd: &mut CommandBuffer,
                  mut state: QueryBorrow<ComponentMut<PhysicsState>>| {
                if let Some(state) = state.first() {
                    for (id, component, _) in removed_rx.try_iter() {
                        let target = component.key().target().expect("joint target is present");
                        let handle = *world.get(id, impulse_joint_handle(target))?;

                        state.detach_joint(handle);
                        cmd.remove(id, impulse_joint_handle(target));
                    }

                    for added in rx.try_iter() {
                        let body1 = *world
                            .get(added.id, rb_handle())
                            .context("Missing rigidbody for joint source")?;

                        let target = added.key.target().expect("joint target is present");
                        let body2 = *world
                            .get(target, rb_handle())
                            .context("Missing rigidbody for joint target")?;

                        let data = world
                            .get(added.id, impulse_joint(target))
                            .context("Missing joint data between entity pairs")?;

                        let handle = state.attach_joint(body1, body2, *data);

                        cmd.set(added.id, impulse_joint_handle(target), handle);
                    }
                }

                anyhow::Ok(())
            },
        )
        .boxed()
}

type UpdateBodiesFetch = (
    Copied<Component<RigidBodyHandle>>,
    <BodyDynamicsQuery as TransformFetch<Modified>>::Output,
);

// writes body data into the physics state
pub fn update_bodies_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new(physics_state().as_mut()))
        .with_query(Query::new((
            rb_handle().copied(),
            BodyDynamicsQuery::new().modified(),
        )))
        .build(
            move |mut state: QueryBorrow<ComponentMut<PhysicsState>>,
                  mut query: QueryBorrow<UpdateBodiesFetch>| {
                if let Some(state) = state.first() {
                    state.update_bodies(query.iter().map(|(rb, v)| {
                        (
                            rb,
                            BodyDynamicsQueryItem {
                                pos: v.pos,
                                rotation: v.rotation,
                                vel: v.vel,
                                ang_vel: v.ang_vel,
                            },
                        )
                    }));
                }

                anyhow::Ok(())
            },
        )
        .boxed()
}

// writes collider position data into the physics state
pub fn update_colliders_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new(physics_state().as_mut()))
        .with_query(
            Query::new((collider_handle().copied(), ColliderDynamicsQuery::new()))
                .without(rb_handle()),
        )
        .build(
            move |mut state: QueryBorrow<ComponentMut<PhysicsState>>,
                  mut query: QueryBorrow<
                (Copied<Component<ColliderHandle>>, ColliderDynamicsQuery),
                _,
            >| {
                if let Some(state) = state.first() {
                    state.update_colliders(query.iter());
                }

                anyhow::Ok(())
            },
        )
        .boxed()
}

pub fn physics_step_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            physics_state().as_mut(),
            gravity().source(engine()),
        )))
        .for_each(|(v, gravity)| {
            v.set_gravity(*gravity);
            v.step();
        })
        .boxed()
}

pub fn sync_simulation_bodies_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new(physics_state().as_mut()))
        .with_query(Query::new(BodyDynamicsQueryMut::new()))
        .build(
            move |mut state: QueryBorrow<ComponentMut<PhysicsState>>,
                  mut query: QueryBorrow<BodyDynamicsQueryMut, _>| {
                if let Some(state) = state.first() {
                    state.sync_body_velocities(&mut query);
                }

                anyhow::Ok(())
            },
        )
        .boxed()
}

#[allow(clippy::type_complexity)]
pub fn gizmo_system(dt: f32) -> BoxedSystem {
    System::builder()
        .with_query(Query::new(ivy_core::components::gizmos()))
        .with_query(
            Query::new((
                world_transform(),
                velocity(),
                angular_velocity(),
                effector(),
            ))
            .without(main_camera()),
        )
        .build(
            move |mut gizmos: QueryBorrow<Component<Gizmos>>,
                  mut query: QueryBorrow<
                (
                    Component<Mat4>,
                    Component<Vec3>,
                    Component<Vec3>,
                    Component<crate::Effector>,
                ),
                _,
            >| {
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

#[allow(clippy::type_complexity)]
pub fn configure_effectors_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new(physics_state()))
        .with_query(Query::new((
            effector().as_mut(),
            (mass(), rb_handle(), inertia_tensor(), center_of_mass()).modified(),
        )))
        .build(
            |mut physics_state: QueryBorrow<'_, Component<PhysicsState>>,
             mut query: QueryBorrow<
                '_,
                (
                    ComponentMut<crate::Effector>,
                    flax::filter::Union<(
                        ChangeFilter<f32>,
                        ChangeFilter<RigidBodyHandle>,
                        ChangeFilter<f32>,
                        ChangeFilter<Vec3>,
                    )>,
                ),
            >| {
                if let Some(physics_state) = physics_state.first() {
                    for (effector, (_, &handle, _, _)) in query.iter() {
                        effector.update_props(physics_state.rigidbody(handle));
                    }
                }
            },
        )
        .boxed()
}

/// Applies effectors to their respective entities and clears the effects.
pub fn apply_effectors_system(dt: f32) -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            velocity().as_mut(),
            angular_velocity().as_mut(),
            effector().as_mut(),
        )))
        .par_for_each(move |(vel, ang_vel, effector)| {
            if effector.should_wake() {
                let net_dv = effector.net_velocity_change(dt);
                *vel += net_dv;

                *ang_vel += effector.net_angular_velocity_change(dt);
            }

            effector.clear();
        })
        .boxed()
}
