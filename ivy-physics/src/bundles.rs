//! This module contains bundles and queries suitable for physics.
use core::f32;

use flax::EntityBuilder;
use glam::Vec3;
use ivy_core::Bundle;
use rapier3d::prelude::{ColliderBuilder, LockedAxes, RigidBodyType, SharedShape};

use crate::{
    components::{
        angular_velocity, can_sleep, collider_builder, effector, inertia_tensor, locked_axes,
        rigid_body_type, velocity,
    },
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
    #[serde(default = "default_fixed")]
    pub body_type: RigidBodyType,
    #[serde(default = "default_true")]
    pub can_sleep: bool,
    #[serde(default)]
    pub mass: f32,
    #[serde(default)]
    pub angular_mass: f32,
    #[serde(default)]
    pub locked_axes: Option<LockedAxes>,

    #[serde(default)]
    pub velocity: Vec3,
    #[serde(default)]
    pub angular_velocity: Vec3,
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

    /// Set the can sleep
    pub fn with_can_sleep(mut self, can_sleep: bool) -> Self {
        self.can_sleep = can_sleep;
        self
    }
}

impl Bundle for RigidBodyBundle {
    fn mount(self, entity: &mut EntityBuilder) {
        entity
            .set(rigid_body_type(), self.body_type)
            .set(velocity(), self.velocity)
            .set(inertia_tensor(), self.angular_mass)
            .set(angular_velocity(), self.angular_velocity)
            .set(effector(), Effector::new());

        entity.set_opt(locked_axes(), self.locked_axes);

        if self.can_sleep {
            entity.set(can_sleep(), ());
        }
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
    fn mount(self, entity: &mut EntityBuilder) {
        entity.set(collider_builder(), self.collider);
    }
}
