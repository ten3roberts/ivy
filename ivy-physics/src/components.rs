use flax::{component, signal::BoxedSignal, Debuggable, Entity};
use glam::Vec3;
use rapier3d::prelude::{
    ColliderBuilder, ColliderHandle, CollisionEvent, CollisionEventFlags, GenericJoint,
    ImpulseJointHandle, LockedAxes, RigidBodyHandle, RigidBodyType,
};

use crate::{state::PhysicsState, Effector};

component! {
    pub physics_state: PhysicsState,
    pub effector: Effector,
    pub rb_handle: RigidBodyHandle,

    pub collider_handle: ColliderHandle,

    pub rigid_body_type: RigidBodyType,
    pub locked_axes: LockedAxes,

    pub collider_builder: ColliderBuilder,

    pub center_of_mass: Vec3 => [ Debuggable ],

    pub can_sleep: (),

    pub velocity: Vec3 => [ Debuggable ],
    pub gravity: Vec3 => [ Debuggable ],
    pub angular_velocity: Vec3 => [ Debuggable ],

    /// Mass of body
    pub mass: f32 => [ Debuggable ],
    pub inertia_tensor: f32 => [ Debuggable ],
    pub gravity_influence: f32 => [ Debuggable ],

    pub sleeping: () => [ Debuggable ],
    pub is_trigger: () => [ Debuggable ],

    pub on_collision_signal: BoxedSignal<EntityCollisionEvent>,
}

// Joints
component! {
    /// impulse based joint from the current entity to the target
    pub impulse_joint(target): GenericJoint,
    pub impulse_joint_handle(target): ImpulseJointHandle,
}

#[derive(Copy, Clone, Hash, Debug)]
pub struct TriggeringCollider {
    pub collider_id: Entity,
    pub collider: ColliderHandle,
    pub rigidbody_id: Entity,
    pub rigidbody: RigidBodyHandle,
}

impl TriggeringCollider {
    pub fn new(
        collider_id: Entity,
        collider: ColliderHandle,
        rigidbody_id: Entity,
        rigidbody: RigidBodyHandle,
    ) -> Self {
        Self {
            collider_id,
            collider,
            rigidbody_id,
            rigidbody,
        }
    }

    pub fn from_collider(state: &PhysicsState, collider_handle: ColliderHandle) -> Self {
        let collider = state.collider(collider_handle);
        let collider_id = Entity::try_from_bits(collider.user_data as u64).unwrap();

        let rigidbody = collider.parent().expect("Collider has no parent");
        let rigidbody_id =
            Entity::try_from_bits(state.rigidbody(rigidbody).user_data as u64).unwrap();

        Self::new(collider_id, collider_handle, rigidbody_id, rigidbody)
    }
}

#[derive(Copy, Clone, Hash, Debug)]
/// Events occurring when two colliders start or stop colliding.
///
/// Contains the associate entities and rigidbody entities for ease of use
pub enum EntityCollisionEvent {
    /// Event occurring when two colliders start colliding
    Started(TriggeringCollider, TriggeringCollider, CollisionEventFlags),
    /// Event occurring when two colliders stop colliding.
    Stopped(TriggeringCollider, TriggeringCollider, CollisionEventFlags),
}

impl EntityCollisionEvent {
    pub fn from_collision_event(state: &PhysicsState, event: CollisionEvent) -> Self {
        match event {
            CollisionEvent::Started(collider1, collider2, flags) => {
                let collider1 = TriggeringCollider::from_collider(state, collider1);
                let collider2 = TriggeringCollider::from_collider(state, collider2);
                Self::Started(collider1, collider2, flags)
            }
            CollisionEvent::Stopped(collider1, collider2, flags) => {
                let collider1 = TriggeringCollider::from_collider(state, collider1);
                let collider2 = TriggeringCollider::from_collider(state, collider2);
                Self::Stopped(collider1, collider2, flags)
            }
        }
    }

    pub fn swap(self) -> Self {
        match self {
            Self::Started(h1, h2, f) => Self::Started(h2, h1, f),
            Self::Stopped(h1, h2, f) => Self::Stopped(h2, h1, f),
        }
    }

    /// Is this a `Started` collision event?
    pub fn started(self) -> bool {
        matches!(self, Self::Started(..))
    }

    /// Is this a `Stopped` collision event?
    pub fn stopped(self) -> bool {
        matches!(self, Self::Stopped(..))
    }

    /// The handle of the first collider involved in this collision event.
    pub fn collider1(self) -> TriggeringCollider {
        match self {
            Self::Started(h, _, _) | Self::Stopped(h, _, _) => h,
        }
    }

    /// The handle of the second collider involved in this collision event.
    pub fn collider2(self) -> TriggeringCollider {
        match self {
            Self::Started(_, h, _) | Self::Stopped(_, h, _) => h,
        }
    }

    /// Was at least one of the colliders involved in the collision a sensor?
    pub fn sensor(self) -> bool {
        match self {
            Self::Started(_, _, f) | Self::Stopped(_, _, f) => {
                f.contains(CollisionEventFlags::SENSOR)
            }
        }
    }

    /// Was at least one of the colliders involved in the collision removed?
    pub fn removed(self) -> bool {
        match self {
            Self::Started(_, _, f) | Self::Stopped(_, _, f) => {
                f.contains(CollisionEventFlags::REMOVED)
            }
        }
    }
}
