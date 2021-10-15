use derive_for::derive_for;
use derive_more::*;
use hecs::*;
use ivy_core::{Position, Rotation, Scale};
use ultraviolet::{Mat4, Vec3};

derive_for!(
    (
        Add,
        Clone,
        Copy,
        AddAssign,
        AsRef,
        Debug,
        Default,
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

    #[repr(transparent)]
    pub struct AngularVelocity(pub Vec3);

    #[repr(transparent)]
    pub struct Mass(pub f32);

    #[repr(transparent)]
    /// The elasticity of the physics material. A high value means that object is
    /// hard and will bounce back. A value of zero means the energy is absorbed.
    pub struct Resitution(pub f32);

);

impl Velocity {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }
}

#[derive(AsRef, Clone, Copy, Debug, Default, Deref, DerefMut, From, Into, Mul, MulAssign)]
#[repr(transparent)]
/// A matrix transforming a point from local space to world space. This can
/// be used to transform a direction relative to the entity to be relative to
/// the world.
pub struct TransformMatrix(pub Mat4);

impl TransformMatrix {
    pub fn new(pos: Position, rot: Rotation, scale: Scale) -> Self {
        Self(
            Mat4::from_translation(*pos)
                * rot.into_matrix().into_homogeneous()
                * Mat4::from_nonuniform_scale(*scale),
        )
    }
}

#[derive(Query)]
pub struct RbQuery<'a> {
    pub pos: &'a Position,
    pub resitution: &'a Resitution,
    pub vel: &'a Velocity,
    pub mass: &'a Mass,
}

/// Manages the forces applied to an entity
#[derive(Clone)]
pub struct Effector {
    net_force: Vec3,
    net_impulse: Vec3,
    net_dv: Vec3,
}

impl Effector {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears all forces affecting the entity
    pub fn clear(&mut self) {
        *self = Self::default()
    }

    pub fn apply_force(&mut self, f: Vec3) {
        self.net_force += f;
    }

    pub fn apply_impulse(&mut self, j: Vec3) {
        self.net_impulse += j
    }

    pub fn apply_velocity_chaneg(&mut self, dv: Vec3) {
        self.net_dv += dv;
    }

    /// Returns the total net effect of forces, impulses, and velocity changes
    /// during `dt`. Note, effector should be clear afterwards.
    pub fn net_effect(&self, mass: Mass, dt: f32) -> Velocity {
        Velocity(self.net_force * dt / mass.0 + self.net_impulse / mass.0 + self.net_dv)
    }
}

impl Default for Effector {
    fn default() -> Self {
        Self {
            net_force: Vec3::default(),
            net_impulse: Vec3::default(),
            net_dv: Vec3::default(),
        }
    }
}
