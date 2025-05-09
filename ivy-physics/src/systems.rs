use anyhow::Context;
use flax::{
    components::child_of,
    entity_ids,
    events::EventSubscriber,
    fetch::{entity_refs, EntityRefs, Modified, Source, TransformFetch, Traverse},
    filter::{All, ChangeFilter, ChangeFilterMut, Without},
    signal::BoxedSignal,
    system, BoxedSystem, CommandBuffer, Component, ComponentMut, EntityIds, FetchExt, Opt, Query,
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
        ColliderBuilder, ColliderHandle, CollisionEvent, LockedAxes, RigidBodyBuilder,
        RigidBodyHandle, RigidBodyType,
    },
};

use crate::{
    components::*,
    state::{
        BodyDynamicsQuery, BodyDynamicsQueryItem, BodyDynamicsQueryMut, ColliderDynamicsQuery,
        ColliderDynamicsQueryItem, PhysicsState,
    },
    Effector,
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

                        let rb_mass = state.rigidbody(rb).mass();
                        cmd.set(id, rb_handle(), rb).set(id, mass(), rb_mass);
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

impl PhysicsState {
    #[system(with_query(Query::new((entity_ids(), collider_builder().added(), TransformQuery::new(), (entity_ids(), rb_handle()).traverse(child_of)))), with_cmd_mut)]
    pub fn register_colliders_system(
        self: &mut PhysicsState,
        query: &mut QueryBorrow<(
            EntityIds,
            ChangeFilter<ColliderBuilder>,
            TransformQuery,
            Source<(EntityIds, Component<RigidBodyHandle>), Traverse>,
        )>,
        cmd: &mut CommandBuffer,
    ) {
        for (id, collider, transform, (parent_id, &parent)) in query {
            let local_position = if parent_id == id {
                Isometry::identity()
            } else {
                let transform: TransformQueryItem = transform;
                Isometry::new(
                    (*transform.pos).into(),
                    transform.rotation.to_scaled_axis().into(),
                )
            };

            let handle = self.attach_collider(
                id,
                collider.clone().position(local_position).build(),
                parent,
            );

            self.recompute_mass(parent);
            let rb = self.rigidbody(parent);
            tracing::info!(
                "Attaching collider {collider:?} to {parent:?} with mass {}",
                rb.mass()
            );
            cmd.set(id, collider_handle(), handle)
                .set(parent_id, mass(), rb.mass())
                .set(parent_id, center_of_mass(), (*rb.center_of_mass()).into());
        }
    }

    /// writes body data into the physics state
    #[system(with_query(Query::new((rb_handle(), BodyDynamicsQuery::new().modified()))))]
    pub(crate) fn update_body_data_system(
        self: &mut PhysicsState,
        query: &mut QueryBorrow<(
            Component<RigidBodyHandle>,
            <BodyDynamicsQuery as TransformFetch<Modified>>::Output,
        )>,
    ) {
        self.update_bodies(query.iter().map(|(&rb, v)| {
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

    /// Write collider position data into the physics state
    #[system(with_query(Query::new((collider_handle(), ColliderDynamicsQuery::new().modified())).without(rb_handle())))]
    pub(crate) fn update_collider_position_system(
        self: &mut PhysicsState,
        query: &mut QueryBorrow<
            (
                Component<ColliderHandle>,
                <ColliderDynamicsQuery as TransformFetch<Modified>>::Output,
            ),
            (All, Without),
        >,
    ) {
        self.update_colliders(query.iter().map(|(&handle, v)| {
            (
                handle,
                ColliderDynamicsQueryItem {
                    pos: v.pos,
                    rotation: v.rotation,
                },
            )
        }));
    }

    #[system]
    pub(crate) fn step_system(self: &mut PhysicsState, gravity: Vec3) {
        self.set_gravity(gravity);
        self.step();
    }

    #[system(with_query(Query::new((entity_refs(), on_collision_signal().as_mut()))), with_cmd_mut)]
    pub(crate) fn process_events_system(
        self: &mut PhysicsState,
        query: &mut QueryBorrow<(EntityRefs, ComponentMut<BoxedSignal<EntityCollisionEvent>>)>,
        cmd: &mut CommandBuffer,
    ) -> anyhow::Result<()> {
        self.process_pending_events(query, cmd)
    }

    #[system(with_query(Query::new(BodyDynamicsQueryMut::new())))]
    pub(crate) fn sync_bodies_after_step_system(
        self: &mut PhysicsState,
        query: &mut QueryBorrow<BodyDynamicsQueryMut>,
    ) {
        self.sync_body_velocities(query);
    }

    #[system(with_query(Query::new((rb_handle(), effector().as_mut().modified()))))]
    pub(crate) fn apply_effectors_system(
        self: &mut PhysicsState,
        query: &mut QueryBorrow<(Component<RigidBodyHandle>, ChangeFilterMut<Effector>)>,
    ) {
        for (&rb, effector) in query {
            let body = self.rigidbody_mut(rb);

            body.add_force(effector.pending_force().into(), effector.should_wake());
            body.add_torque(effector.pending_torque().into(), effector.should_wake());

            body.apply_impulse(
                (effector.pending_impulse() + effector.pending_velocity_change() * body.mass())
                    .into(),
                effector.should_wake(),
            );
            body.apply_torque_impulse(
                effector.pending_torque_impulse().into(),
                effector.should_wake(),
            );

            effector.clear();
        }
    }
}

#[allow(clippy::type_complexity)]
pub fn gizmo_system() -> BoxedSystem {
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

                    let dv = effector.pending_force();
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
