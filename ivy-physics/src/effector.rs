use glam::Vec3;
use nalgebra::{Matrix3, Vector, Vector3};
use rapier3d::{parry::utils::SdpMatrix3, prelude::RigidBody};

/// Manages the forces applied to an entity.
/// Stored in the entity and is a middle hand for manipulating velocity and
/// angular velocity through direct changes, forces, and impulses. It is
/// recommended to change forces through the effector due to the stacking effect
/// and non requirement of knowing the dt.
///
/// It is also possible to create a dummy effector to "record" physics effects.
#[derive(Clone, Debug, PartialEq)]
pub struct Effector {
    dv: Vec3,
    // working with a sdp is easier in nalgebra types
    dw: Vector3<f32>,
    instant_dv: Vec3,
    instant_dw: Vector3<f32>,
    inv_mass: Vec3,
    inertia_tensor_sqrt: SdpMatrix3<f32>,
    translation: Vec3,
    wake: bool,
}

impl Effector {
    pub fn new() -> Self {
        Self {
            inv_mass: Vec3::ZERO,
            dv: Vec3::ZERO,
            dw: Vector3::new(0.0, 0.0, 0.0),
            instant_dv: Vec3::ZERO,
            instant_dw: Default::default(),
            translation: Vec3::ZERO,
            wake: false,
            inertia_tensor_sqrt: SdpMatrix3::from_sdp_matrix(Matrix3::identity()),
        }
    }

    pub fn update_props(&mut self, rb: &RigidBody) {
        self.inertia_tensor_sqrt = rb.mass_properties().effective_world_inv_inertia_sqrt;
        self.inv_mass = rb.mass_properties().effective_inv_mass.into();
    }

    pub fn should_wake(&self) -> bool {
        self.wake
    }

    pub fn wake(&mut self) {
        self.wake = true
    }

    pub fn apply_other(&mut self, other: &Self) {
        self.dv += other.dv;
        self.instant_dv += other.instant_dv;
        self.dw += other.dw;
        self.instant_dw += other.instant_dw;
    }

    /// Clears all forces affecting the entity
    pub fn clear(&mut self) {
        *self = Self {
            dv: Vec3::ZERO,
            dw: Default::default(),
            instant_dv: Vec3::ZERO,
            instant_dw: Default::default(),
            translation: Vec3::ZERO,
            inv_mass: self.inv_mass,
            inertia_tensor_sqrt: self.inertia_tensor_sqrt,
            wake: false,
        }
    }

    pub fn apply_torque(&mut self, torque: Vec3) {
        self.dw += self.inertia_tensor_sqrt * (self.inertia_tensor_sqrt * Vector::from(torque));
    }

    pub fn apply_angular_impulse(&mut self, j: Vec3) {
        self.instant_dw += self.inertia_tensor_sqrt * (self.inertia_tensor_sqrt * Vector::from(j));
    }

    pub fn apply_angular_velocity_change(&mut self, dw: Vec3) {
        self.instant_dw += Vector3::from(dw);
    }

    pub fn apply_angular_acceleration(&mut self, dw: Vec3) {
        self.dw += Vector3::from(dw);
    }

    pub fn apply_force(&mut self, f: Vec3, wake: bool) {
        self.dv += f * self.inv_mass;
        self.wake = self.wake || wake;
    }

    /// Applies an instantaneous force
    pub fn apply_impulse(&mut self, j: Vec3, wake: bool) {
        self.instant_dv += j * self.inv_mass;
        self.wake = self.wake || wake;
    }

    /// Applies a continous acceleration independent of mass
    pub fn apply_acceleration(&mut self, dv: Vec3, wake: bool) {
        self.dv += dv;
        self.wake = self.wake || wake
    }

    /// Applies a force at the specified position from center of mass
    pub fn apply_force_at(&mut self, f: Vec3, at: Vec3, wake: bool) {
        self.apply_force(f, wake);
        self.apply_torque(at.cross(f));
    }

    /// Applies an impulse at the specified position from center of mass
    pub fn apply_impulse_at(&mut self, impulse: Vec3, point: Vec3, wake: bool) {
        self.apply_impulse(impulse, wake);
        self.apply_angular_impulse(point.cross(impulse));
    }

    pub fn apply_velocity_change(&mut self, dv: Vec3, wake: bool) {
        self.instant_dv += dv;
        self.wake = self.wake || wake;
    }

    /// Applies a velocity change at the specified position from center of mass
    pub fn apply_velocity_change_at(&mut self, dv: Vec3, at: Vec3, wake: bool) {
        self.apply_velocity_change(dv, wake);
        self.apply_angular_velocity_change(at.cross(dv))
    }

    pub fn translate(&mut self, translate: Vec3) {
        self.translation += translate;
    }

    /// Returns the total net effect of forces, impulses, and velocity changes
    /// during `dt`. Note, Effector should be clear afterwards.
    pub fn net_velocity_change(&self, dt: f32) -> Vec3 {
        self.dv * dt + self.instant_dv
    }

    /// Returns the total net effect of torques, angular impulses, and angular
    /// velocity changes. Note: Effector should be cleared afterwards.
    pub fn net_angular_velocity_change(&self, dt: f32) -> Vec3 {
        (self.dw * dt + self.instant_dw).into()
    }

    /// Get the effector's translation.
    pub fn translation(&self) -> Vec3 {
        self.translation
    }
}

impl Default for Effector {
    fn default() -> Self {
        Self::new()
    }
}
