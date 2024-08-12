//! This module contains bundles and queries suitable for physics.
use flax::{Component, EntityBuilder, Fetch, Mutable};
use glam::Vec3;
use ivy_core::{angular_mass, angular_velocity, friction, mass, restitution, velocity, Bundle};

use crate::components::effector;

#[derive(Fetch)]
pub struct RbQuery {
    pub restitution: Component<f32>,
    pub vel: Component<Vec3>,
    pub ang_vel: Component<Vec3>,
    pub mass: Component<f32>,
    pub ang_mass: Component<f32>,
    pub friction: Component<f32>,
}

impl RbQuery {
    pub fn new() -> Self {
        Self {
            restitution: restitution(),
            vel: velocity(),
            ang_vel: angular_velocity(),
            mass: mass(),
            ang_mass: angular_mass(),
            friction: friction(),
        }
    }
}

impl Default for RbQuery {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default, Debug)]
/// Bundle for all things neccessary for all things physics
pub struct RbBundle {
    pub velocity: Vec3,
    pub mass: f32,
    pub angular_mass: f32,
    pub angular_velocity: Vec3,
    pub restitution: f32,
    pub friction: f32,
}

impl RbBundle {
    pub fn new(
        mass: f32,
        vel: Vec3,
        ang_vel: Vec3,
        ang_mass: f32,
        resitution: f32,
        friction: f32,
    ) -> Self {
        Self {
            velocity: vel,
            mass,
            angular_velocity: ang_vel,
            angular_mass: ang_mass,
            restitution: resitution,
            friction,
        }
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
}

impl Bundle for RbBundle {
    fn mount(self, entity: &mut EntityBuilder) {
        entity
            .set(velocity(), self.velocity)
            .set(mass(), self.mass)
            .set(angular_mass(), self.angular_mass)
            .set(angular_velocity(), self.angular_velocity)
            .set(restitution(), self.restitution)
            .set(friction(), self.friction)
            .set(effector(), Default::default());
    }
}

#[derive(Fetch)]
pub struct RbQueryMut {
    pub resitution: Mutable<f32>,
    pub vel: Mutable<Vec3>,
    pub ang_vel: Mutable<Vec3>,
    pub mass: Mutable<f32>,
    pub ang_mass: Mutable<f32>,
    pub friction: Mutable<f32>,
}

impl Default for RbQueryMut {
    fn default() -> Self {
        Self::new()
    }
}

impl RbQueryMut {
    pub fn new() -> Self {
        Self {
            resitution: restitution().as_mut(),
            vel: velocity().as_mut(),
            ang_vel: angular_velocity().as_mut(),
            mass: mass().as_mut(),
            ang_mass: angular_mass().as_mut(),
            friction: friction().as_mut(),
        }
    }
}
