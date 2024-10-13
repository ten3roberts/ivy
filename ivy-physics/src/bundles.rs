//! This module contains bundles and queries suitable for physics.
use core::f32;

use flax::EntityBuilder;
use glam::Vec3;
use ivy_core::{
    components::{angular_velocity, inertia_tensor, mass, velocity},
    Bundle,
};
use rapier3d::prelude::{RigidBodyType, SharedShape};

use crate::{
    components::{
        can_sleep, collider_shape, density, effector, friction, restitution, rigid_body_type,
    },
    Effector,
};

#[derive(Debug)]
/// Bundle for a rigidbody without collider
pub struct RigidBodyBundle {
    pub body_type: RigidBodyType,
    pub can_sleep: bool,
    pub mass: f32,
    pub angular_mass: f32,

    pub velocity: Vec3,
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

    /// Set the mass
    pub fn with_mass(mut self, mass: f32) -> Self {
        self.mass = mass;
        self
    }

    /// Set the velocity
    pub fn with_velocity(mut self, velocity: Vec3) -> Self {
        self.velocity = velocity;
        self
    }

    /// Set the ang mass
    pub fn with_angular_mass(mut self, angular_mass: f32) -> Self {
        self.angular_mass = angular_mass;
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
            .set(mass(), self.mass)
            .set(inertia_tensor(), self.angular_mass)
            .set(angular_velocity(), self.angular_velocity)
            .set(effector(), Effector::new());

        if self.can_sleep {
            entity.set(can_sleep(), ());
        }
    }
}

pub struct ColliderBundle {
    shape: SharedShape,
    density: f32,
    friction: f32,
    restitution: f32,
}

impl ColliderBundle {
    pub fn new(shape: SharedShape) -> Self {
        Self {
            shape,
            density: 1.0,
            friction: 0.0,
            restitution: 0.0,
        }
    }

    /// Set the restitution
    pub fn with_restitution(mut self, restitution: f32) -> Self {
        self.restitution = restitution;
        self
    }

    /// Set the friction
    pub fn with_friction(mut self, friction: f32) -> Self {
        self.friction = friction;
        self
    }

    /// Set the density
    pub fn with_density(mut self, density: f32) -> Self {
        self.density = density;
        self
    }
}

impl Bundle for ColliderBundle {
    fn mount(self, entity: &mut EntityBuilder) {
        entity
            .set(collider_shape(), self.shape)
            .set(density(), self.density)
            .set(restitution(), self.restitution)
            .set(friction(), self.friction);
    }
}
