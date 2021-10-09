use derive_for::derive_for;
use derive_more::*;
use ivy_core::{Position, Rotation, Scale};
use ultraviolet::{Mat4, Vec3};

derive_for!(
    (
        Add,
        AddAssign,
        AsRef,
        Clone,
        Copy,
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

);

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
