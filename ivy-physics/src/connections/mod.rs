use derive_for::*;
use derive_more::*;
use ivy_base::{TransformBundle, TransformQueryMut};

mod systems;
pub use systems::*;
use ultraviolet::Vec3;

use crate::{
    components::{RbBundle, RbQuery, RbQueryMut, Velocity},
    util::point_vel,
};

derive_for!(
    (
        Add,
        AddAssign,
        AsRef,
        Clone,
        Copy,
        Debug,
        Deref,
        DerefMut,
        Div,
        DivAssign,
        Sub,
        SubAssign,
        From,
        Into,
        Mul,
        MulAssign,
        Default,
        PartialEq,
    );
    /// Describes the offset of the entity from the parent
    pub struct OffsetPosition(pub Vec3);
);

impl OffsetPosition {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }
}

/// Marker type for two physically connected objects.
pub struct Connection;

pub enum ConnectionKind {
    /// Connection will not budge
    Rigid,
}

impl ConnectionKind {
    fn update(
        &self,
        offset: &OffsetPosition,
        child_trans: TransformQueryMut,
        child_rb: RbQueryMut,
        parent_trans: &TransformBundle,
        parent_rb: &RbBundle,
    ) {
        let pos = parent_trans.into_matrix().transform_point3(**offset);
        let vel = point_vel(pos - *parent_trans.pos, parent_rb.ang_vel);
        match self {
            Self::Rigid => {
                *child_trans.pos = pos.into();
                *child_rb.vel = Velocity(vel) + parent_rb.vel;
                *child_rb.ang_vel = parent_rb.ang_vel;
                *child_trans.rot = parent_trans.rot;
            }
        }
    }
}
