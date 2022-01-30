use glam::Vec3;
use ivy_base::{math::Inverse, AngularMass, AngularVelocity, Mass, Position, Velocity};

/// Manages the forces applied to an entity.
/// Stored in the entity and is a middle hand for manipulating velocity and
/// angular velocity through direct changes, forces, and impulses. It is
/// recommended to change forces through the effector due to the stacking effect
/// and non requirement of knowing the dt.
///
/// It is also possible to create a dummy effector to "record" physics effects.
#[derive(Default, Clone, Debug, PartialEq)]
pub struct Effector {
    dv: Vec3,
    dw: Vec3,
    instant_dv: Vec3,
    instant_dw: Vec3,
    inv_mass: f32,
    inv_ang_mass: f32,
    translation: Vec3,
}

impl Effector {
    pub fn new(mass: Mass, ang_mass: AngularMass) -> Self {
        Self {
            inv_mass: mass.inv(),
            inv_ang_mass: ang_mass.inv(),
            dv: Vec3::ZERO,
            dw: Vec3::ZERO,
            instant_dv: Vec3::ZERO,
            instant_dw: Vec3::ZERO,
            translation: Vec3::ZERO,
        }
    }

    pub fn apply_other(&mut self, other: &Self) {
        self.dv += other.dv;
        self.instant_dv += other.instant_dv;
        self.dw += other.dw;
        self.instant_dw += other.instant_dw;
    }

    pub fn set_mass(&mut self, mass: Mass) {
        self.inv_mass = mass.inv();
    }

    pub fn set_ang_mass(&mut self, ang_mass: AngularMass) {
        self.inv_ang_mass = ang_mass.inv()
    }

    /// Clears all forces affecting the entity
    pub fn clear(&mut self) {
        *self = Self {
            dv: Vec3::ZERO,
            dw: Vec3::ZERO,
            instant_dv: Vec3::ZERO,
            instant_dw: Vec3::ZERO,
            translation: Vec3::ZERO,
            inv_mass: self.inv_mass,
            inv_ang_mass: self.inv_ang_mass,
        }
    }

    pub fn apply_torque(&mut self, torque: Vec3) {
        self.dw += torque * self.inv_ang_mass;
    }

    pub fn apply_angular_impulse(&mut self, j: Vec3) {
        self.instant_dw += j * self.inv_ang_mass
    }

    pub fn apply_angular_velocity_change(&mut self, dw: Vec3) {
        self.instant_dw += dw
    }

    pub fn apply_angular_acceleration(&mut self, dw: Vec3) {
        self.dw += dw
    }

    /// Applies a continuos force using mass
    pub fn apply_force(&mut self, f: Vec3) {
        self.dv += f * self.inv_mass
    }

    /// Applies an instantaneous force
    pub fn apply_impulse(&mut self, j: Vec3) {
        self.instant_dv += j * self.inv_mass
    }

    /// Applies a continous acceleration independent of mass
    pub fn apply_acceleration(&mut self, dv: Vec3) {
        self.dv += dv;
    }

    /// Applies a force at the specified position from center of mass
    pub fn apply_force_at(&mut self, f: Vec3, at: Position) {
        self.apply_force(f);
        self.apply_torque(at.cross(f));
    }

    /// Applies an impulse at the specified position from center of mass
    pub fn apply_impulse_at(&mut self, impulse: Vec3, at: Position) {
        self.apply_impulse(impulse);
        self.apply_angular_impulse(at.cross(impulse));
    }

    pub fn apply_velocity_change(&mut self, dv: Vec3) {
        self.instant_dv += dv
    }

    /// Applies a velocity change at the specified position from center of mass
    pub fn apply_velocity_change_at(&mut self, dv: Vec3, at: Position) {
        self.apply_velocity_change(dv);
        self.apply_angular_velocity_change(at.cross(dv))
    }

    pub fn translate(&mut self, translate: Vec3) {
        self.translation += translate;
    }

    /// Returns the total net effect of forces, impulses, and velocity changes
    /// during `dt`. Note, Effector should be clear afterwards.
    pub fn net_velocity_change(&self, dt: f32) -> Velocity {
        Velocity(self.dv * dt + self.instant_dv)
    }
    /// Returns the total net effect of torques, angular impulses, and angular
    /// velocity changes. Note: Effector should be cleared afterwards.

    pub fn net_angular_velocity_change(&self, dt: f32) -> AngularVelocity {
        AngularVelocity(self.dw * dt + self.instant_dw)
    }

    /// Get the effector's translation.
    pub fn translation(&self) -> Position {
        Position(self.translation)
    }
}
