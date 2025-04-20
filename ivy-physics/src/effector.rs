use glam::Vec3;

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

    pending_force: Vec3,
    pending_torque: Vec3,
    pending_impulse: Vec3,
    pending_torque_impulse: Vec3,
    wake: bool,
}

impl Effector {
    pub fn new() -> Self {
        Self {
            dv: Vec3::ZERO,
            wake: false,
            pending_force: Vec3::ZERO,
            pending_torque: Vec3::ZERO,
            pending_impulse: Vec3::ZERO,
            pending_torque_impulse: Vec3::ZERO,
        }
    }

    pub fn should_wake(&self) -> bool {
        self.wake
    }

    pub fn wake(&mut self) {
        self.wake = true
    }

    /// Clears all forces affecting the entity
    pub fn clear(&mut self) {
        *self = Self {
            dv: Vec3::ZERO,
            wake: false,
            pending_force: Vec3::ZERO,
            pending_torque: Vec3::ZERO,
            pending_impulse: Vec3::ZERO,
            pending_torque_impulse: Vec3::ZERO,
        }
    }

    pub fn apply_torque(&mut self, torque: Vec3) {
        self.pending_torque += torque;
    }

    pub fn apply_torque_impulse(&mut self, j: Vec3) {
        self.pending_torque_impulse += j;
    }

    pub fn apply_force(&mut self, f: Vec3, wake: bool) {
        self.pending_force += f;
        self.wake = self.wake || wake;
    }

    /// Applies an instantaneous force
    pub fn apply_impulse(&mut self, j: Vec3, wake: bool) {
        self.pending_impulse += j;
        self.wake = self.wake || wake;
    }

    /// Applies a continuous acceleration independent of mass
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
        self.apply_torque_impulse(point.cross(impulse));
    }

    pub fn pending_force(&self) -> Vec3 {
        self.pending_force
    }

    pub fn pending_impulse(&self) -> Vec3 {
        self.pending_impulse
    }

    pub fn pending_torque(&self) -> Vec3 {
        self.pending_torque
    }

    pub fn pending_torque_impulse(&self) -> Vec3 {
        self.pending_torque_impulse
    }
}

impl Default for Effector {
    fn default() -> Self {
        Self::new()
    }
}
