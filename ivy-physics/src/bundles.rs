//! This module contains bundles and queries suitable for physics.
use crate::Effector;
use hecs::{Bundle, DynamicBundleClone, Query};
use ivy_base::{AngularMass, AngularVelocity, Friction, Mass, Resitution, Velocity};
use ivy_collision::Collider;

#[derive(Query, Clone, Copy, Debug, PartialEq)]
pub struct RbQuery<'a> {
    pub resitution: &'a Resitution,
    pub vel: &'a Velocity,
    pub ang_vel: &'a AngularVelocity,
    pub mass: &'a Mass,
    pub ang_mass: &'a AngularMass,
    pub friction: &'a Friction,
}

#[derive(Default, Bundle, Debug, DynamicBundleClone)]
/// Bundle for all things neccessary for all things physics
pub struct RbBundle {
    pub vel: Velocity,
    pub mass: Mass,
    pub ang_mass: AngularMass,
    pub ang_vel: AngularVelocity,
    pub resitution: Resitution,
    pub friction: Friction,
    pub effector: Effector,
}

impl RbBundle {
    pub fn new(
        mass: Mass,
        vel: Velocity,
        ang_vel: AngularVelocity,
        ang_mass: AngularMass,
        resitution: Resitution,
        friction: Friction,
    ) -> Self {
        Self {
            vel,
            mass,
            ang_vel,
            ang_mass,
            resitution,
            effector: Default::default(),
            friction,
        }
    }

    /// Get a reference to the rb bundle's vel.
    pub fn vel(&self) -> Velocity {
        self.vel
    }
}

/// Same as [ `crate::RbBundle` ] but also contains a collider.
#[derive(Default, Bundle, Debug, DynamicBundleClone)]
pub struct RbColliderBundle {
    pub vel: Velocity,
    pub mass: Mass,
    pub ang_mass: AngularMass,
    pub ang_vel: AngularVelocity,
    pub resitution: Resitution,
    pub effector: Effector,
    pub collider: Collider,
    pub friction: Friction,
}

impl RbColliderBundle {
    pub fn new(
        mass: Mass,
        vel: Velocity,
        ang_vel: AngularVelocity,
        ang_mass: AngularMass,
        resitution: Resitution,
        collider: Collider,
        friction: Friction,
    ) -> Self {
        Self {
            vel,
            mass,
            ang_vel,
            ang_mass,
            resitution,
            effector: Default::default(),
            collider: collider.into(),
            friction,
        }
    }
}

#[derive(Query, PartialEq)]
pub struct RbQueryMut<'a> {
    pub resitution: &'a mut Resitution,
    pub vel: &'a mut Velocity,
    pub ang_vel: &'a mut AngularVelocity,
    pub mass: &'a mut Mass,
    pub ang_mass: &'a mut AngularMass,
    pub friction: &'a mut Friction,
}
