use core::f32;

use derive_for::derive_for;
use derive_more::*;
use ivy_random::Random;
use ultraviolet::Vec3;

derive_for!(
    (
        PartialEq,
        Add,
        Clone,
        Copy,
        AddAssign,
        AsRef,
        Debug,
        Deref,
        DerefMut,
        From,
        Into,
        Mul,
        MulAssign,
        Sub,
        SubAssign,
        Div, DivAssign,
        Neg
    );
    #[derive(Default)]
    #[repr(transparent)]
    pub struct Velocity(pub Vec3);

    /// Represents and angular velocity in xyz directions creating an axis and
    /// magnitude.
    #[derive(Default)]
    #[repr(transparent)]
    pub struct AngularVelocity(pub Vec3);

    #[repr(transparent)]
    pub struct Mass(pub f32);

    #[repr(transparent)]
    /// Moment of intertia; angular mass and resistance to torque.
    pub struct AngularMass(pub f32);

    #[derive(Default)]
    #[repr(transparent)]
    /// The elasticity of the physics material. A high value means that object is
    /// hard and will bounce back. A value of zero means the energy is absorbed.
    pub struct Resitution(pub f32);

);

impl Default for Mass {
    fn default() -> Self {
        Self(1.0)
    }
}

impl Default for AngularMass {
    fn default() -> Self {
        Self(1.0)
    }
}
// impl AngularVelocity {
//     /// Creates an angular velocity from an axis angle rotation. Note: Axis is
//     /// assumed to be normalized.
//     pub fn axis_angle(angle: f32, axis: Vec3) -> Self {
//         Self(Rotor3::new(angle, Bivec3::from_normalized_axis(axis)))
//     }
// }

impl Velocity {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }
}

impl AngularVelocity {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }
}

/// Manages the forces applied to an entity.
/// Stored in the entity and is a middle hand for manipulating velocity and
/// angular velocity through direct changes, forces, and impulses. It is
/// recommended to change forces through the effector due to the stacking effect
/// and non requirement of knowing the dt.
///
/// It is also possible to create a dummy effector to "record" physics effects.
#[derive(Clone, Debug, PartialEq)]
pub struct Effector {
    force: Vec3,
    impulse: Vec3,
    delta_v: Vec3,

    torque: Vec3,
    angular_impulse: Vec3,
    delta_w: Vec3,
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
    pub fn apply_force_at(&mut self, f: Vec3, at: Vec3) {
        self.force += f;
        self.torque += at.cross(f);
    }

    /// Applies an impulse at the specified position from center of mass
    pub fn apply_impulse_at(&mut self, impulse: Vec3, at: Vec3) {
        self.impulse += impulse;
        self.angular_impulse += at.cross(impulse);
    }

    /// Applies a velocity change at the specified position from center of mass
    pub fn apply_velocity_change_at(&mut self, dv: Vec3, at: Vec3) {
        self.delta_v += dv;
        self.delta_w += at.cross(dv);
    }

    /// Returns the total net effect of forces, impulses, and velocity changes
    /// during `dt`. Note, Effector should be clear afterwards.
    pub fn net_velocity_change(&self, mass: Mass, dt: f32) -> Velocity {
        Velocity(self.force * dt / mass.0 + self.impulse / mass.0 + self.delta_v)
    }
    /// Returns the total net effect of torques, angular impulses, and angular
    /// velocity changes. Note: Effector should be cleared afterwards.

    pub fn net_angular_velocity_change(&self, ang_mass: AngularMass, dt: f32) -> AngularVelocity {
        AngularVelocity(
            self.torque * dt / ang_mass.0 + self.angular_impulse / ang_mass.0 + self.delta_w,
        )
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

impl Default for Effector {
    fn default() -> Self {
        Self {
            torque: Vec3::default(),
            angular_impulse: Vec3::default(),
            delta_w: Vec3::default(),
            force: Vec3::default(),
            impulse: Vec3::default(),
            delta_v: Vec3::default(),
        }
    }
}

impl Random for Velocity {
    fn rand_unit<R: ivy_random::rand::Rng>(rng: &mut R) -> Self {
        Velocity(Vec3::rand_unit(rng))
    }

    fn rand_sphere<R: ivy_random::rand::Rng>(rng: &mut R) -> Self {
        Velocity(Vec3::rand_sphere(rng))
    }

    fn rand_constrained_sphere<R: ivy_random::rand::Rng>(rng: &mut R, r1: f32, r2: f32) -> Self {
        Velocity(Vec3::rand_constrained_sphere(rng, r1, r2))
    }

    fn rand_uniform<R: ivy_random::rand::Rng>(rng: &mut R) -> Self {
        Velocity(Vec3::rand_uniform(rng))
    }
}

impl Random for AngularVelocity {
    fn rand_unit<R: ivy_random::rand::Rng>(rng: &mut R) -> Self {
        AngularVelocity(Vec3::rand_unit(rng))
    }

    fn rand_sphere<R: ivy_random::rand::Rng>(rng: &mut R) -> Self {
        AngularVelocity(Vec3::rand_sphere(rng))
    }

    fn rand_constrained_sphere<R: ivy_random::rand::Rng>(rng: &mut R, r1: f32, r2: f32) -> Self {
        AngularVelocity(Vec3::rand_constrained_sphere(rng, r1, r2))
    }

    fn rand_uniform<R: ivy_random::rand::Rng>(rng: &mut R) -> Self {
        AngularVelocity(Vec3::rand_uniform(rng))
    }
}
