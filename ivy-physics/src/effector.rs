use glam::Vec3;
use ivy_base::{math::Inverse, AngularMass, AngularVelocity, Mass, Position, Rotation, Velocity};

/// Manages the forces applied to an entity.
/// Stored in the entity and is a middle hand for manipulating velocity and
/// angular velocity through direct changes, forces, and impulses. It is
/// recommended to change forces through the effector due to the stacking effect
/// and non requirement of knowing the dt.
///
/// It is also possible to create a dummy effector to "record" physics effects.
#[derive(Default, Clone, Debug, PartialEq)]
pub struct Effector {
    force: Vec3,
    impulse: Vec3,
    delta_v: Vec3,

    torque: Vec3,
    angular_impulse: Vec3,
    delta_w: Vec3,
    translate: Vec3,
    local_translate: Vec3,
}

impl Effector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears all forces affecting the entity
    pub fn clear(&mut self) {
        *self = Self::default()
    }

    pub fn apply_torque(&mut self, torque: Vec3) {
        self.torque += torque;
    }

    pub fn apply_angular_impulse(&mut self, j: Vec3) {
        self.angular_impulse += j;
    }

    pub fn apply_angular_velocity_change(&mut self, dw: Vec3) {
        self.delta_w += dw;
    }

    pub fn apply_force(&mut self, f: Vec3) {
        self.force += f;
    }

    pub fn apply_impulse(&mut self, j: Vec3) {
        self.impulse += j
    }

    pub fn apply_velocity_change(&mut self, dv: Vec3) {
        self.delta_v += dv;
    }

    /// Applies a force at the specified position from center of mass
    pub fn apply_force_at(&mut self, f: Vec3, at: Position) {
        self.force += f;
        self.torque += at.cross(f);
    }

    /// Applies an impulse at the specified position from center of mass
    pub fn apply_impulse_at(&mut self, impulse: Vec3, at: Position) {
        self.impulse += impulse;
        self.angular_impulse += at.cross(impulse);
    }

    /// Applies a velocity change at the specified position from center of mass
    pub fn apply_velocity_change_at(&mut self, dv: Vec3, at: Position) {
        self.delta_v += dv;
        self.delta_w += at.cross(dv);
    }

    pub fn translate(&mut self, translate: Vec3) {
        self.translate += translate;
    }

    pub fn translate_local(&mut self, translate: Vec3) {
        self.local_translate += translate;
    }

    /// Returns the total net effect of forces, impulses, and velocity changes
    /// during `dt`. Note, Effector should be clear afterwards.
    pub fn net_velocity_change(&self, mass: Mass, dt: f32) -> Velocity {
        Velocity(self.force * dt * mass.inv() + self.impulse * mass.inv() + self.delta_v)
    }
    /// Returns the total net effect of torques, angular impulses, and angular
    /// velocity changes. Note: Effector should be cleared afterwards.

    pub fn net_angular_velocity_change(&self, ang_mass: AngularMass, dt: f32) -> AngularVelocity {
        AngularVelocity(
            self.torque * dt * ang_mass.inv()
                + self.angular_impulse * ang_mass.inv()
                + self.delta_w,
        )
    }

    pub fn net_translation(&self, rotation: &Rotation) -> Position {
        Position(self.translate + rotation.mul_vec3(self.local_translate))
    }
}

impl std::ops::Add<Effector> for Effector {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self {
            force: self.force + rhs.force,
            impulse: self.impulse + rhs.impulse,
            delta_v: self.delta_v + rhs.delta_v,
            torque: self.torque + rhs.torque,
            angular_impulse: self.angular_impulse + rhs.angular_impulse,
            delta_w: self.delta_w + rhs.delta_w,
            translate: self.translate + rhs.translate,
            local_translate: self.local_translate + rhs.local_translate,
        }
    }
}

impl std::ops::AddAssign<Effector> for Effector {
    fn add_assign(&mut self, rhs: Self) {
        self.force += rhs.force;
        self.impulse += rhs.impulse;
        self.delta_v += rhs.delta_v;
        self.torque += rhs.torque;
        self.angular_impulse += rhs.angular_impulse;
        self.delta_w += rhs.delta_w;
        self.translate += rhs.translate;
        self.local_translate += rhs.local_translate;
    }
}

impl std::ops::Add<&Effector> for Effector {
    type Output = Self;

    fn add(self, rhs: &Self) -> Self::Output {
        Self {
            force: self.force + rhs.force,
            impulse: self.impulse + rhs.impulse,
            delta_v: self.delta_v + rhs.delta_v,
            torque: self.torque + rhs.torque,
            angular_impulse: self.angular_impulse + rhs.angular_impulse,
            delta_w: self.delta_w + rhs.delta_w,
            translate: self.translate + rhs.translate,
            local_translate: self.local_translate + rhs.local_translate,
        }
    }
}

impl std::ops::AddAssign<&Effector> for Effector {
    fn add_assign(&mut self, rhs: &Self) {
        self.force += rhs.force;
        self.impulse += rhs.impulse;
        self.delta_v += rhs.delta_v;
        self.torque += rhs.torque;
        self.angular_impulse += rhs.angular_impulse;
        self.delta_w += rhs.delta_w;
    }
}
