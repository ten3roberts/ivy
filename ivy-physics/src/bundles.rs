//! This module contains bundles and queries suitable for physics.
use core::f32;

use flax::EntityBuilder;
use glam::Vec3;
use ivy_core::Bundle;
use rapier3d::prelude::{
    ColliderBuilder, LockedAxes, RigidBodyBuilder, RigidBodyType, SharedShape,
};

use crate::{
    components::{
        angular_velocity, collider_builder, effector, inertia_tensor, rigidbody_builder,
        rigidbody_flags, velocity,
    },
    state::RigidBodyFlags,
    Effector,
};

fn default_fixed() -> RigidBodyType {
    RigidBodyType::Fixed
}

fn default_true() -> bool {
    true
}

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
/// Bundle for a rigidbody without collider
pub struct RigidBodyBundle {
    #[cfg_attr(feature = "serde", serde(default = "default_fixed"))]
    pub body_type: RigidBodyType,
    #[cfg_attr(feature = "serde", serde(default = "default_true"))]
    pub can_sleep: bool,
    #[cfg_attr(feature = "serde", serde(default))]
    pub mass: f32,
    #[cfg_attr(feature = "serde", serde(default))]
    pub angular_mass: f32,
    #[cfg_attr(feature = "serde", serde(default))]
    pub locked_axes: Option<LockedAxes>,

    #[cfg_attr(feature = "serde", serde(default))]
    pub velocity: Vec3,
    #[cfg_attr(feature = "serde", serde(default))]
    pub angular_velocity: Vec3,

    #[cfg_attr(feature = "serde", serde(default))]
    pub linear_damping: f32,

    #[cfg_attr(feature = "serde", serde(default))]
    pub angular_damping: f32,
}

impl RigidBodyBundle {
    pub fn new(body_type: RigidBodyType) -> Self {
        Self {
            body_type,
            velocity: Vec3::ZERO,
            mass: 0.0,
            angular_velocity: Vec3::ZERO,
            angular_mass: 0.0,
            can_sleep: true,
            locked_axes: Default::default(),
            linear_damping: 0.0,
            angular_damping: 0.0,
        }
    }

    pub fn dynamic() -> Self {
        Self::new(RigidBodyType::Dynamic)
    }

    pub fn kinematic_position() -> Self {
        Self::new(RigidBodyType::KinematicPositionBased)
    }

    pub fn kinematic_velocity() -> Self {
        Self::new(RigidBodyType::KinematicVelocityBased)
    }

    pub fn fixed() -> Self {
        Self::new(RigidBodyType::Fixed)
    }

    pub fn with_locked_axes(mut self, axes: LockedAxes) -> Self {
        self.locked_axes = Some(axes);
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
                RigidBodyBuilder::new(self.body_type)
                    .additional_mass(self.mass)
                    .can_sleep(self.can_sleep)
                    .gravity_scale(1.0)
                    .locked_axes(self.locked_axes.unwrap_or(LockedAxes::empty()))
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

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
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
