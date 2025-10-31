//! This module contains bundles and queries suitable for physics.
use core::f32;

use flax::EntityBuilder;
use glam::{BVec3, Vec3};
use ivy_assets::Resource;
use ivy_core::Bundle;
use ivy_editable::Editable;
use rapier3d::prelude::{ColliderBuilder, LockedAxes, RigidBodyBuilder, SharedShape};

use crate::{
    components::{
        angular_velocity, collider_builder, effector, inertia_tensor, rigidbody_builder,
        rigidbody_flags, velocity,
    },
    state::RigidBodyFlags,
    Effector,
};

fn default_fixed() -> RigidBodyKind {
    RigidBodyKind::Fixed
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug, Resource, Bundle, serde::Serialize, serde::Deserialize)]
#[resource(derive = [Editable])]
/// Bundle for a rigidbody without collider
pub struct RigidBodyBundle {
    #[resource_attr(editable(default = RigidBodyKind::Dynamic))]
    #[serde(default = "default_fixed")]
    pub body_type: RigidBodyKind,
    #[serde(default = "default_true")]
    #[resource_attr(editable(default = true))]
    pub can_sleep: bool,
    #[serde(default)]
    #[resource_attr(editable(default))]
    /// Additional mass added to the body.
    pub mass: f32,
    #[serde(default)]
    #[resource_attr(editable(default))]
    pub angular_mass: f32,
    #[serde(default)]
    #[resource_attr(editable(default))]
    /// Constrain the movement of the body
    pub constraints: AxisContraints,

    #[serde(default)]
    #[resource_attr(editable(default))]
    pub velocity: Vec3,
    #[serde(default)]
    #[resource_attr(editable(default))]
    pub angular_velocity: Vec3,

    #[serde(default)]
    #[resource_attr(editable(default))]
    pub linear_damping: f32,

    #[serde(default)]
    #[resource_attr(editable(default))]
    pub angular_damping: f32,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq, Editable, serde::Serialize, serde::Deserialize)]
/// The status of a body, governing the way it is affected by external forces.
pub enum RigidBodyKind {
    /// A `RigidBodyType::Dynamic` body can be affected by all external forces.
    Dynamic = 0,
    /// A `RigidBodyType::Fixed` body cannot be affected by external forces.
    Fixed = 1,
    /// A `RigidBodyType::KinematicPositionBased` body cannot be affected by any external forces but can be controlled
    /// by the user at the position level while keeping realistic one-way interaction with dynamic bodies.
    ///
    /// One-way interaction means that a kinematic body can push a dynamic body, but a kinematic body
    /// cannot be pushed by anything. In other words, the trajectory of a kinematic body can only be
    /// modified by the user and is independent from any contact or joint it is involved in.
    KinematicPositionBased = 2,
    /// A `RigidBodyType::KinematicVelocityBased` body cannot be affected by any external forces but can be controlled
    /// by the user at the velocity level while keeping realistic one-way interaction with dynamic bodies.
    ///
    /// One-way interaction means that a kinematic body can push a dynamic body, but a kinematic body
    /// cannot be pushed by anything. In other words, the trajectory of a kinematic body can only be
    /// modified by the user and is independent from any contact or joint it is involved in.
    KinematicVelocityBased = 3,
    // Semikinematic, // A kinematic that performs automatic CCD with the fixed environment to avoid traversing it?
    // Disabled,
}

#[derive(Debug, Clone, Copy, Editable, serde::Serialize, serde::Deserialize)]
pub struct AxisContraints {
    lock_translation: BVec3,
    lock_rotation: BVec3,
}

impl Default for AxisContraints {
    fn default() -> Self {
        Self {
            lock_translation: BVec3::FALSE,
            lock_rotation: BVec3::FALSE,
        }
    }
}

impl AxisContraints {
    pub fn new(translation: BVec3, rotation: BVec3) -> Self {
        Self {
            lock_translation: translation,
            lock_rotation: rotation,
        }
    }
}

impl From<AxisContraints> for LockedAxes {
    fn from(value: AxisContraints) -> Self {
        LockedAxes::from_bits(
            (value.lock_translation.bitmask() | (value.lock_rotation.bitmask() << 3)) as u8,
        )
        .expect("Invalid LockedAxes bits")
    }
}

impl From<RigidBodyKind> for rapier3d::prelude::RigidBodyType {
    fn from(value: RigidBodyKind) -> Self {
        match value {
            RigidBodyKind::Dynamic => rapier3d::prelude::RigidBodyType::Dynamic,
            RigidBodyKind::Fixed => rapier3d::prelude::RigidBodyType::Fixed,
            RigidBodyKind::KinematicPositionBased => {
                rapier3d::prelude::RigidBodyType::KinematicPositionBased
            }
            RigidBodyKind::KinematicVelocityBased => {
                rapier3d::prelude::RigidBodyType::KinematicVelocityBased
            }
        }
    }
}

impl From<rapier3d::prelude::RigidBodyType> for RigidBodyKind {
    fn from(value: rapier3d::prelude::RigidBodyType) -> Self {
        match value {
            rapier3d::prelude::RigidBodyType::Dynamic => RigidBodyKind::Dynamic,
            rapier3d::prelude::RigidBodyType::Fixed => RigidBodyKind::Fixed,
            rapier3d::prelude::RigidBodyType::KinematicPositionBased => {
                RigidBodyKind::KinematicPositionBased
            }
            rapier3d::prelude::RigidBodyType::KinematicVelocityBased => {
                RigidBodyKind::KinematicVelocityBased
            }
        }
    }
}

impl RigidBodyBundle {
    pub fn new(body_type: RigidBodyKind) -> Self {
        Self {
            body_type,
            velocity: Vec3::ZERO,
            mass: 0.0,
            angular_velocity: Vec3::ZERO,
            angular_mass: 0.0,
            can_sleep: true,
            constraints: Default::default(),
            linear_damping: 0.0,
            angular_damping: 0.0,
        }
    }

    pub fn dynamic() -> Self {
        Self::new(RigidBodyKind::Dynamic)
    }

    pub fn kinematic_position() -> Self {
        Self::new(RigidBodyKind::KinematicPositionBased)
    }

    pub fn kinematic_velocity() -> Self {
        Self::new(RigidBodyKind::KinematicVelocityBased)
    }

    pub fn fixed() -> Self {
        Self::new(RigidBodyKind::Fixed)
    }

    pub fn with_axis_constraints(mut self, constraints: AxisContraints) -> Self {
        self.constraints = constraints;
        self
    }

    /// Set the velocity
    pub fn with_velocity(mut self, velocity: Vec3) -> Self {
        self.velocity = velocity;
        self
    }

    /// Set the angular velocity
    pub fn with_angular_velocity(mut self, angular_velocity: Vec3) -> Self {
        self.angular_velocity = angular_velocity;
        self
    }

    pub fn with_mass(mut self, mass: f32) -> Self {
        self.mass = mass;
        self
    }

    /// Set the inertia tensor
    pub fn with_inertia_tensor(mut self, inertia_tensor: f32) -> Self {
        self.angular_mass = inertia_tensor;
        self
    }

    /// Set the can sleep
    pub fn with_can_sleep(mut self, can_sleep: bool) -> Self {
        self.can_sleep = can_sleep;
        self
    }

    pub fn with_linear_damping(mut self, linear_damping: f32) -> Self {
        self.linear_damping = linear_damping;
        self
    }

    pub fn with_angular_damping(mut self, angular_damping: f32) -> Self {
        self.angular_damping = angular_damping;
        self
    }
}

impl Bundle for RigidBodyBundle {
    fn mount(&self, entity: &mut EntityBuilder) {
        entity
            .set(
                rigidbody_builder(),
                RigidBodyBuilder::new(self.body_type.into())
                    .additional_mass(self.mass)
                    .can_sleep(self.can_sleep)
                    .gravity_scale(1.0)
                    .locked_axes(self.constraints.into())
                    .linear_damping(self.linear_damping)
                    .angular_damping(self.angular_damping),
            )
            .set(velocity(), self.velocity)
            .set(inertia_tensor(), self.angular_mass)
            .set(angular_velocity(), self.angular_velocity)
            .set(effector(), Effector::new())
            .set(rigidbody_flags(), RigidBodyFlags::new());
    }
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ColliderBundle {
    collider: ColliderBuilder,
}

impl ColliderBundle {
    pub fn new(shape: SharedShape) -> Self {
        Self {
            collider: ColliderBuilder::new(shape),
        }
    }

    pub fn from_builder(builder: ColliderBuilder) -> Self {
        Self { collider: builder }
    }

    /// Set the restitution
    pub fn with_restitution(mut self, restitution: f32) -> Self {
        self.collider.restitution = restitution;
        self
    }

    /// Set the friction
    pub fn with_friction(mut self, friction: f32) -> Self {
        self.collider.friction = friction;
        self
    }

    /// Set the density
    pub fn with_density(mut self, density: f32) -> Self {
        self.collider = self.collider.density(density);
        self
    }
}

impl Bundle for ColliderBundle {
    fn mount(&self, entity: &mut EntityBuilder) {
        entity.set(collider_builder(), self.collider.clone());
    }
}
