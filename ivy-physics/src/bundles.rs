//! This module contains bundles and queries suitable for physics.
use crate::components::{AngularMass, AngularVelocity, Effector, Mass, Resitution, Velocity};
use hecs::{Bundle, Query};
use ivy_collision::Collider;

#[derive(Query, Clone, Copy, Debug, PartialEq)]
pub struct RbQuery<'a> {
    pub resitution: &'a Resitution,
    pub vel: &'a Velocity,
    pub ang_vel: &'a AngularVelocity,
    pub mass: &'a Mass,
    pub ang_mass: &'a AngularMass,
    pub effector: &'a Effector,
}

impl<'a> RbQuery<'a> {
    pub fn into_owned(&self) -> RbBundle {
        RbBundle {
            resitution: *self.resitution,
            vel: *self.vel,
            ang_vel: *self.ang_vel,
            mass: *self.mass,
            ang_mass: *self.ang_mass,
            effector: self.effector.clone(),
        }
    }
}

#[derive(Default, Bundle, Debug)]
/// Bundle for all things neccessary for all things physics
pub struct RbBundle {
    pub vel: Velocity,
    pub mass: Mass,
    pub ang_mass: AngularMass,
    pub ang_vel: AngularVelocity,
    pub resitution: Resitution,
    pub effector: Effector,
}

impl RbBundle {
    pub fn new(
        mass: Mass,
        vel: Velocity,
        ang_vel: AngularVelocity,
        ang_mass: AngularMass,
        resitution: Resitution,
    ) -> Self {
        Self {
            vel,
            mass,
            ang_vel,
            ang_mass,
            resitution,
            effector: Default::default(),
        }
    }
}

/// Same as [ `crate::RbBundle` ] but also contains a collider.
#[derive(Default, Bundle, Debug)]
pub struct RbColliderBundle {
    pub vel: Velocity,
    pub mass: Mass,
    pub ang_mass: AngularMass,
    pub ang_vel: AngularVelocity,
    pub resitution: Resitution,
    pub effector: Effector,
    pub collider: Collider,
}

impl RbColliderBundle {
    pub fn new(
        mass: Mass,
        vel: Velocity,
        ang_vel: AngularVelocity,
        ang_mass: AngularMass,
        resitution: Resitution,
        collider: Collider,
    ) -> Self {
        Self {
            vel,
            mass,
            ang_vel,
            ang_mass,
            resitution,
            effector: Default::default(),
            collider: collider.into(),
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
}
