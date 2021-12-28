use core::f32;

use derive_for::derive_for;
use derive_more::*;
use ivy_random::Random;
#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};
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
        Neg,
        Default
    );
    #[repr(transparent)]
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub struct Velocity(pub Vec3);

    #[repr(transparent)]
    /// The strength of gravity
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub struct Gravity(pub Vec3);
    /// Represents and angular velocity in xyz directions creating an axis and
    /// magnitude.
    #[repr(transparent)]
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub struct AngularVelocity(pub Vec3);

    #[repr(transparent)]
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub struct Mass(pub f32);

    #[repr(transparent)]
    /// Moment of intertia; angular mass and resistance to torque.
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub struct AngularMass(pub f32);

    #[repr(transparent)]
    /// How strongly the entity is affected by gravity, is at all
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub struct GravityInfluence(pub f32);

    #[repr(transparent)]
    /// The elasticity of the physics material. A high value means that object is
    /// hard and will bounce back. A value of zero means the energy is absorbed.
    #[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
    pub struct Resitution(pub f32);

);

impl Velocity {
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }

    pub const fn zero() -> Self {
        Self(Vec3::new(0.0, 0.0, 0.0))
    }
}

impl Gravity {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }
}

impl AngularVelocity {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
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
