use core::f32;

use derive_for::derive_for;
use derive_more::*;
use hecs::*;
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
    );
    #[repr(transparent)]
    pub struct Velocity(pub Vec3);

    /// Represents and angular velocity in xyz directions creating an axis and
    /// magnitude.
    #[repr(transparent)]
    pub struct AngularVelocity(pub Vec3);

    #[repr(transparent)]
    pub struct Mass(pub f32);

    #[repr(transparent)]
    /// Moment of intertia; angular mass and resistance to torque.
    pub struct AngularMass(pub f32);

    #[repr(transparent)]
    /// The elasticity of the physics material. A high value means that object is
    /// hard and will bounce back. A value of zero means the energy is absorbed.
    pub struct Resitution(pub f32);

);

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

#[derive(Query, Clone, Copy, Debug, PartialEq)]
pub struct RbQuery<'a> {
    pub resitution: &'a Resitution,
    pub vel: &'a Velocity,
    pub ang_vel: &'a AngularVelocity,
    pub mass: &'a Mass,
    pub ang_mass: &'a AngularMass,
}

impl<'a> RbQuery<'a> {
    pub fn into_owned(&self) -> RbBundle {
        RbBundle {
            resitution: *self.resitution,
            vel: *self.vel,
            ang_vel: *self.ang_vel,
            mass: *self.mass,
            ang_mass: *self.ang_mass,
        }
    }
}

#[derive(Bundle, Clone, Copy, Debug, PartialEq)]
pub struct RbBundle {
    pub resitution: Resitution,
    pub vel: Velocity,
    pub ang_vel: AngularVelocity,
    pub mass: Mass,
    pub ang_mass: AngularMass,
}

#[derive(Query, PartialEq)]
pub struct RbQueryMut<'a> {
    pub resitution: &'a mut Resitution,
    pub vel: &'a mut Velocity,
    pub ang_vel: &'a mut AngularVelocity,
    pub mass: &'a mut Mass,
    pub ang_mass: &'a mut AngularMass,
}

/// Manages the forces applied to an entity
#[derive(Clone)]
pub struct Effector {
    net_force: Vec3,
    net_impulse: Vec3,
    net_dv: Vec3,

    net_torque: Vec3,
    net_angular_impulse: Vec3,
    net_dw: Vec3,
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
        self.net_torque += torque;
    }

    pub fn apply_angular_impulse(&mut self, j: Vec3) {
        self.net_angular_impulse += j;
    }

    pub fn apply_angular_velocity_change(&mut self, dw: Vec3) {
        self.net_dw += dw;
    }

    pub fn apply_force(&mut self, f: Vec3) {
        self.net_force += f;
    }

    pub fn apply_impulse(&mut self, j: Vec3) {
        self.net_impulse += j
    }

    pub fn apply_velocity_change(&mut self, dv: Vec3) {
        self.net_dv += dv;
    }

    /// Applies a force at the specified position from center of mass
    pub fn apply_force_at(&mut self, f: Vec3, at: Vec3) {
        self.net_force += f;
        self.net_torque += at.cross(f);
    }

    /// Applies an impulse at the specified position from center of mass
    pub fn apply_impulse_at(&mut self, impulse: Vec3, at: Vec3) {
        self.net_impulse += impulse;
        self.net_angular_impulse += at.cross(impulse);
    }

    /// Applies a velocity change at the specified position from center of mass
    pub fn apply_velocity_change_at(&mut self, dv: Vec3, at: Vec3) {
        self.net_dv += dv;
        self.net_dw += at.cross(dv);
    }

    /// Returns the total net effect of forces, impulses, and velocity changes
    /// during `dt`. Note, Effector should be clear afterwards.
    pub fn net_velocity_change(&self, mass: Mass, dt: f32) -> Velocity {
        Velocity(self.net_force * dt / mass.0 + self.net_impulse / mass.0 + self.net_dv)
    }
    /// Returns the total net effect of torques, angular impulses, and angular
    /// velocity changes. Note: Effector should be cleared afterwards.

    pub fn net_angular_velocity_change(&self, ang_mass: AngularMass, dt: f32) -> AngularVelocity {
        AngularVelocity(
            self.net_torque * dt / ang_mass.0 + self.net_angular_impulse / ang_mass.0 + self.net_dw,
        )
    }
}

impl Default for Effector {
    fn default() -> Self {
        Self {
            net_torque: Vec3::default(),
            net_angular_impulse: Vec3::default(),
            net_dw: Vec3::default(),
            net_force: Vec3::default(),
            net_impulse: Vec3::default(),
            net_dv: Vec3::default(),
        }
    }
}

impl Default for Velocity {
    fn default() -> Self {
        Self(Vec3::default())
    }
}

impl Default for AngularVelocity {
    fn default() -> Self {
        Self(Vec3::default())
    }
}

impl Default for Mass {
    fn default() -> Self {
        Self(f32::MAX / 4.0)
    }
}

impl Default for AngularMass {
    fn default() -> Self {
        Self(f32::MAX / 4.0)
    }
}

impl Default for Resitution {
    fn default() -> Self {
        Self(0.0)
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
