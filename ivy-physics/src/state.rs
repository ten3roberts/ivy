use flax::{
    fetch::EntityRefs, signal::BoxedSignal, CommandBuffer, Component, ComponentMut, Entity, Fetch,
    QueryBorrow,
};
use glam::{Quat, Vec3};
use ivy_core::components::{position, rotation};
use nalgebra::Isometry3;
use rapier3d::{
    crossbeam,
    prelude::{
        CCDSolver, ChannelEventCollector, Collider, ColliderHandle, ColliderSet, CollisionEvent,
        ContactForceEvent, DefaultBroadPhase, GenericJoint, ImpulseJointHandle, ImpulseJointSet,
        IntegrationParameters, IslandManager, MultibodyJointSet, NarrowPhase, PhysicsPipeline,
        QueryFilter, QueryPipeline, Ray, RayIntersection, RigidBody, RigidBodyHandle, RigidBodySet,
    },
};

use crate::components::{angular_velocity, velocity, EntityCollisionEvent};

#[derive(Debug, Clone)]
pub struct RaycastHit {
    pub rigidbody_id: Entity,
    pub collider_id: Entity,
    pub collider: ColliderHandle,
    pub intersection: RayIntersection,
}

impl RaycastHit {
    pub fn new(
        id: Entity,
        root_id: Entity,
        collider: ColliderHandle,
        intersection: RayIntersection,
    ) -> Self {
        Self {
            collider_id: id,
            rigidbody_id: root_id,
            collider,
            intersection,
        }
    }
}

#[derive(Default)]
pub struct PhysicsStateConfiguration {}

pub struct PhysicsState {
    gravity: Vec3,
    bodies: RigidBodySet,
    collider_set: ColliderSet,
    physics_pipeline: PhysicsPipeline,
    island_manager: IslandManager,
    broad_phase: DefaultBroadPhase,
    narrow_phase: NarrowPhase,
    joint_set: ImpulseJointSet,
    multibody_joints: MultibodyJointSet,
    ccd_solder: CCDSolver,
    query_pipeline: QueryPipeline,
    collisions_rx: crossbeam::channel::Receiver<CollisionEvent>,
    forces_rx: crossbeam::channel::Receiver<ContactForceEvent>,
    dt: f32,
    event_collector: ChannelEventCollector,
}

impl PhysicsState {
    pub fn new(_: &PhysicsStateConfiguration, dt: f32) -> Self {
        let (collisions_tx, collisions_rx) = crossbeam::channel::unbounded();
        let (forces_tx, forces_rx) = crossbeam::channel::unbounded();

        let event_collector = ChannelEventCollector::new(collisions_tx, forces_tx);

        Self {
            dt,
            bodies: RigidBodySet::new(),
            collider_set: ColliderSet::new(),
            physics_pipeline: PhysicsPipeline::new(),
            island_manager: IslandManager::new(),
            broad_phase: DefaultBroadPhase::new(),
            narrow_phase: NarrowPhase::new(),
            joint_set: ImpulseJointSet::new(),
            multibody_joints: MultibodyJointSet::new(),
            ccd_solder: CCDSolver::new(),
            query_pipeline: QueryPipeline::new(),
            gravity: -Vec3::Y * 9.81,
            collisions_rx,
            forces_rx,
            event_collector,
        }
    }

    pub fn set_gravity(&mut self, gravity: Vec3) {
        self.gravity = gravity;
    }

    pub fn add_body(&mut self, id: Entity, mut rb: RigidBody) -> RigidBodyHandle {
        rb.user_data = id.as_bits() as u128;
        self.bodies.insert(rb)
    }

    pub fn remove_body(&mut self, rb_handle: RigidBodyHandle) {
        self.bodies.remove(
            rb_handle,
            &mut self.island_manager,
            &mut self.collider_set,
            &mut self.joint_set,
            &mut self.multibody_joints,
            true,
        );
    }

    pub fn rigidbody(&self, handle: RigidBodyHandle) -> &RigidBody {
        &self.bodies[handle]
    }

    pub fn rigidbody_mut(&mut self, handle: RigidBodyHandle) -> &mut RigidBody {
        &mut self.bodies[handle]
    }

    pub fn collider(&self, handle: ColliderHandle) -> &Collider {
        &self.collider_set[handle]
    }

    pub fn collider_parent(&self, handle: ColliderHandle) -> Entity {
        let rb = self.collider_set[handle]
            .parent()
            .expect("Collider must have a parent");
        Entity::try_from_bits(self.bodies[rb].user_data as _).unwrap()
    }

    pub fn remvoe_collider(&mut self, handle: ColliderHandle) {
        self.collider_set
            .remove(handle, &mut self.island_manager, &mut self.bodies, true);
    }

    pub fn attached_rigidbody(&self, collider: ColliderHandle) -> Option<Entity> {
        let handle = self.collider_set.get(collider)?.parent()?;
        Some(Entity::try_from_bits(self.rigidbody(handle).user_data as _).unwrap())
    }

    pub fn attach_collider(
        &mut self,
        id: Entity,
        mut collider: rapier3d::prelude::Collider,
        rb: RigidBodyHandle,
    ) -> ColliderHandle {
        collider.user_data = id.as_bits() as u128;
        self.collider_set
            .insert_with_parent(collider, rb, &mut self.bodies)
    }

    pub fn recompute_mass(&mut self, rb: RigidBodyHandle) {
        self.bodies[rb].recompute_mass_properties_from_colliders(&self.collider_set);
    }

    /// Attach a joint connecting two bodies
    pub fn attach_joint(
        &mut self,
        body1: RigidBodyHandle,
        body2: RigidBodyHandle,
        joint: impl Into<GenericJoint>,
    ) -> ImpulseJointHandle {
        let joint = joint.into();
        self.joint_set.insert(body1, body2, joint, true)
    }

    pub fn detach_joint(&mut self, joint: ImpulseJointHandle) {
        self.joint_set.remove(joint, true);
    }

    pub fn cast_ray(
        &self,
        ray: &Ray,
        max_dist: f32,
        solid: bool,
        filter: QueryFilter,
    ) -> Option<RaycastHit> {
        self.query_pipeline
            .cast_ray_and_get_normal(
                &self.bodies,
                &self.collider_set,
                ray,
                max_dist,
                solid,
                filter,
            )
            .map(|(handle, v)| {
                let collider = &self.collider_set[handle];
                let root = collider.parent().unwrap();
                let id = Entity::try_from_bits(collider.user_data as u64)
                    .expect("user_data is valid entity");
                let root_id = Entity::try_from_bits(self.bodies[root].user_data as u64).unwrap();
                RaycastHit::new(id, root_id, handle, v)
            })
    }

    pub fn cast_ray_many(
        &self,
        ray: &Ray,
        max_dist: f32,
        solid: bool,
        filter: QueryFilter,
        mut callback: impl FnMut(RaycastHit) -> bool,
    ) {
        self.query_pipeline.intersections_with_ray(
            &self.bodies,
            &self.collider_set,
            ray,
            max_dist,
            solid,
            filter,
            |handle, intersect| {
                let collider = &self.collider_set[handle];
                let root = collider.parent().unwrap();
                let id = Entity::try_from_bits(collider.user_data as u64)
                    .expect("user_data is valid entity");
                let root_id = Entity::try_from_bits(self.bodies[root].user_data as u64).unwrap();
                callback(RaycastHit::new(id, root_id, handle, intersect))
            },
        )
    }

    pub fn step(&mut self) {
        assert!(
            self.collisions_rx.is_empty(),
            "Collision events not processed"
        );

        let params = IntegrationParameters {
            dt: self.dt,
            min_ccd_dt: self.dt / 100.0,
            ..Default::default()
        };

        self.physics_pipeline.step(
            &self.gravity.into(),
            &params,
            &mut self.island_manager,
            &mut self.broad_phase,
            &mut self.narrow_phase,
            &mut self.bodies,
            &mut self.collider_set,
            &mut self.joint_set,
            &mut self.multibody_joints,
            &mut self.ccd_solder,
            Some(&mut self.query_pipeline),
            &(),
            &self.event_collector,
        );
    }

    pub fn process_pending_events(
        &mut self,
        signals: &mut QueryBorrow<(EntityRefs, ComponentMut<BoxedSignal<EntityCollisionEvent>>)>,
        cmd: &mut CommandBuffer,
    ) -> anyhow::Result<()> {
        for event in self.collisions_rx.try_iter() {
            for (id1, id2) in [
                (event.collider1(), event.collider2()),
                (event.collider2(), event.collider1()),
            ] {
                let id1 = Entity::try_from_bits(self.collider_set[id1].user_data as u64).unwrap();
                let id2 = Entity::try_from_bits(self.collider_set[id2].user_data as u64).unwrap();

                if let Ok((entity, signal)) = signals.get(id1) {
                    signal.execute(
                        entity,
                        cmd,
                        EntityCollisionEvent::from_collision_event(self, event),
                    )?;
                }

                if let Ok((entity, signal)) = signals.get(id2) {
                    signal.execute(
                        entity,
                        cmd,
                        EntityCollisionEvent::from_collision_event(self, event).swap(),
                    )?;
                }
            }
        }

        // TODO: force events
        self.forces_rx.try_iter().for_each(|_| {});

        Ok(())
    }

    pub fn update_bodies<'x, I>(&mut self, data: I)
    where
        I: Iterator<Item = (RigidBodyHandle, BodyDynamicsQueryItem<'x>)>,
    {
        for (rb_handle, v) in data {
            let rb = &mut self.bodies[rb_handle];

            rb.set_position(
                Isometry3::new((*v.pos).into(), v.rotation.to_scaled_axis().into()),
                false,
            );

            rb.set_linvel((*v.vel).into(), true);
            rb.set_angvel((*v.ang_vel).into(), true);
        }
    }

    pub fn update_colliders<'x, I>(&mut self, data: I)
    where
        I: Iterator<Item = (ColliderHandle, ColliderDynamicsQueryItem<'x>)>,
    {
        for (handle, v) in data {
            let collider = &mut self.collider_set[handle];

            collider.set_position_wrt_parent(Isometry3::new(
                (*v.pos).into(),
                v.rotation.to_scaled_axis().into(),
            ));
        }
    }

    pub fn sync_body_velocities(&mut self, query: &mut QueryBorrow<BodyDynamicsQueryMut>) {
        let bodies = self
            .island_manager
            .active_dynamic_bodies()
            .iter()
            .chain(self.island_manager.active_kinematic_bodies());

        for body in bodies {
            let rb = &self.bodies[*body];
            let id = Entity::try_from_bits(rb.user_data as u64).unwrap();
            let v = query.get(id).unwrap();

            *v.pos = rb.position().translation.into();
            *v.rotation = rb.position().rotation.into();
            *v.vel = (*rb.linvel()).into();
            *v.ang_vel = (*rb.angvel()).into();
        }
    }
}

#[derive(Fetch)]
pub struct BodyDynamicsQueryMut {
    pub pos: ComponentMut<Vec3>,
    pub rotation: ComponentMut<Quat>,
    pub vel: ComponentMut<Vec3>,
    pub ang_vel: ComponentMut<Vec3>,
}

impl Default for BodyDynamicsQueryMut {
    fn default() -> Self {
        Self::new()
    }
}

impl BodyDynamicsQueryMut {
    pub fn new() -> Self {
        Self {
            pos: position().as_mut(),
            rotation: rotation().as_mut(),
            vel: velocity().as_mut(),
            ang_vel: angular_velocity().as_mut(),
        }
    }
}

#[derive(Fetch)]
#[fetch(item_derives = [Debug], transforms = [Modified])]
pub struct BodyDynamicsQuery {
    pub pos: Component<Vec3>,
    pub rotation: Component<Quat>,
    pub vel: Component<Vec3>,
    pub ang_vel: Component<Vec3>,
}

impl BodyDynamicsQuery {
    pub fn new() -> Self {
        Self {
            pos: position(),
            rotation: rotation(),
            vel: velocity(),
            ang_vel: angular_velocity(),
        }
    }
}

impl Default for BodyDynamicsQuery {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Fetch)]
#[fetch(transforms=[Modified])]
pub struct ColliderDynamicsQuery {
    pub pos: Component<Vec3>,
    pub rotation: Component<Quat>,
}

impl ColliderDynamicsQuery {
    pub fn new() -> Self {
        Self {
            pos: position(),
            rotation: rotation(),
        }
    }
}

impl Default for ColliderDynamicsQuery {
    fn default() -> Self {
        Self::new()
    }
}
