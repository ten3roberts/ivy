//! This module contains bundles and queries suitable for physics.
use flax::{Component, EntityBuilder, Fetch, Mutable};
use glam::Vec3;
use ivy_base::{angular_mass, angular_velocity, friction, mass, restitution, velocity};

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

#[derive(Default, Debug)]
/// Bundle for all things neccessary for all things physics
pub struct RbBundle {
    pub vel: Vec3,
    pub mass: f32,
    pub ang_mass: f32,
    pub ang_vel: Vec3,
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
            vel,
            mass,
            ang_vel,
            ang_mass,
            restitution: resitution,
            friction,
        }
    }

    pub fn mount(self, entity: &mut EntityBuilder) {
        entity
            .set(velocity(), self.vel)
            .set(mass(), self.mass)
            .set(angular_mass(), self.ang_mass)
            .set(angular_velocity(), self.ang_vel)
            .set(restitution(), self.restitution)
            .set(friction(), self.friction);
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
