use flax::{component, Debuggable};
use glam::Vec3;
use rapier3d::prelude::{
    ColliderHandle, GenericJoint, ImpulseJointHandle, RigidBodyHandle, RigidBodyType, SharedShape,
};

use crate::{state::PhysicsState, Effector};

component! {
    pub physics_state: PhysicsState,
    pub effector: Effector,
    pub rb_handle: RigidBodyHandle,

    pub collider_handle: ColliderHandle,

    pub rigid_body_type: RigidBodyType,
    pub collider_shape: SharedShape,
    // density of a collider, used to calculate mass
    pub density: f32 => [ Debuggable ],
    /// The elasticity of the physics material
    pub restitution: f32 => [ Debuggable ],
    /// Coefficient of friction
    pub friction: f32 => [ Debuggable ],

    pub center_of_mass: Vec3 => [ Debuggable ],

    pub can_sleep: (),

    pub velocity: Vec3 => [ Debuggable ],
    pub gravity: Vec3 => [ Debuggable ],
    pub angular_velocity: Vec3 => [ Debuggable ],

    pub mass: f32 => [ Debuggable ],
    pub inertia_tensor: f32 => [ Debuggable ],
    pub gravity_influence: f32 => [ Debuggable ],

    pub sleeping: () => [ Debuggable ],
    pub is_trigger: () => [ Debuggable ],
}

// Joints
component! {
    /// impulse based joint from the current entity to the target
    pub impulse_joint(target): GenericJoint,
    pub impulse_joint_handle(target): ImpulseJointHandle,
}
