use ivy_core::components::{
    angular_velocity, is_static, is_trigger, mass, position, rotation, velocity, world_transform,
};
use nalgebra::Isometry3;
use rapier3d::prelude::{
    CCDSolver, ColliderHandle, ColliderSet, DefaultBroadPhase, GenericJoint, GenericJointBuilder,
    ImpulseJointHandle, ImpulseJointSet, IntegrationParameters, IslandManager, MultibodyJointSet,
    NarrowPhase, PhysicsPipeline, QueryFilter, QueryPipeline, Ray, RayIntersection, RigidBody,
    RigidBodyHandle, RigidBodySet, SpringJointBuilder,
};

use flax::{
    fetch::Satisfied, Component, Entity, Fetch, FetchExt, Mutable, Opt, OptOr, QueryBorrow,
};
use glam::{Mat4, Quat, Vec3};
use ivy_collision::{
    components::{collider, collider_offset},
    Collider,
};

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
    dt: f32,
}

impl PhysicsState {
    pub fn new(_: &PhysicsStateConfiguration, dt: f32) -> Self {
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
    ) -> Option<(Entity, ColliderHandle, RayIntersection)> {
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
                let id = Entity::try_from_bits(self.collider_set[handle].user_data as u64)
                    .expect("user_data is valid entity");
                (id, handle, v)
            })
    }

    pub fn cast_ray_many(
        &self,
        ray: &Ray,
        max_dist: f32,
        solid: bool,
        filter: QueryFilter,
        mut callback: impl FnMut(Entity, ColliderHandle, RayIntersection) -> bool,
    ) {
        self.query_pipeline.intersections_with_ray(
            &self.bodies,
            &self.collider_set,
            ray,
            max_dist,
            solid,
            filter,
            |handle, intersect| {
                let id = Entity::try_from_bits(self.collider_set[handle].user_data as u64)
                    .expect("user_data is valid entity");
                callback(id, handle, intersect)
            },
        )
    }

    pub fn step(&mut self) {
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
            &(),
        );
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

            rb.set_linvel((*v.vel).into(), false);
            rb.set_angvel((*v.ang_vel).into(), false);
        }
    }

    pub fn sync_body_velocities(&mut self, query: &mut QueryBorrow<BodyDynamicsQueryMut>) {
        for body in self.island_manager.active_dynamic_bodies() {
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
    pub pos: Mutable<Vec3>,
    pub rotation: Mutable<Quat>,
    pub vel: Mutable<Vec3>,
    pub ang_vel: Mutable<Vec3>,
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
struct ObjectQuery {
    pub transform: Component<Mat4>,
    pub mass: Opt<Component<f32>>,
    pub collider: Component<Collider>,
    pub offset: OptOr<Component<Mat4>, Mat4>,
    pub is_static: Satisfied<Component<()>>,
    pub is_trigger: Satisfied<Component<()>>,
    pub velocity: Component<Vec3>,
    pub angular_velocity: Component<Vec3>,
}

impl ObjectQuery {
    fn new() -> Self {
        Self {
            transform: world_transform(),
            mass: mass().opt(),
            collider: collider(),
            offset: collider_offset().opt_or_default(),
            is_static: is_static().satisfied(),
            velocity: velocity(),
            angular_velocity: angular_velocity(),
            is_trigger: is_trigger().satisfied(),
        }
    }
}

impl Default for ObjectQuery {
    fn default() -> Self {
        Self::new()
    }
}
