use derive_for::*;
use derive_more::*;
use ivy_base::{TransformBundle, TransformQueryMut};

mod systems;
pub use systems::*;
use ultraviolet::{Bivec3, Rotor3, Vec3};

use crate::{
    components::{Effector, RbBundle, RbQuery, RbQueryMut, Velocity},
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
    pub struct OffsetRotation(pub Rotor3);
);

impl OffsetPosition {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }
}

impl OffsetRotation {
    pub fn euler_angles(roll: f32, pitch: f32, yaw: f32) -> Self {
        Self(Rotor3::from_euler_angles(roll, pitch, yaw))
    }
    /// Creates an angular velocity from an axis angle rotation. Note: Axis is
    /// assumed to be normalized.
    pub fn axis_angle(angle: f32, axis: Vec3) -> Self {
        Self(Rotor3::new(angle, Bivec3::from_normalized_axis(axis)))
    }
}

/// Marker type for two physically connected objects.
pub struct Connection;

pub enum ConnectionKind {
    /// Connection will not budge
    Rigid,
    /// The connection will excert a force to return to the desired position
    Spring { strength: f32, dampening: f32 },
}

impl ConnectionKind {
    fn update(
        &self,
        offset_pos: &OffsetPosition,
        offset_rot: &OffsetRotation,
        child_trans: TransformQueryMut,
        child_rb: RbQueryMut,
        parent_trans: &TransformBundle,
        parent_rb: &RbBundle,
        effector: &mut Effector,
    ) {
        let pos = parent_trans.into_matrix().transform_point3(**offset_pos);
        match self {
            Self::Rigid => {
                let vel = point_vel(pos - *parent_trans.pos, parent_rb.ang_vel);
                *child_trans.pos = pos.into();
                *child_rb.vel = Velocity(vel) + parent_rb.vel;
                *child_rb.ang_vel = parent_rb.ang_vel;
                *child_trans.rot = parent_trans.rot * **offset_rot;
            }
            Self::Spring {
                strength,
                dampening,
            } => {
                let displacement = pos - **child_trans.pos;
                effector.apply_force(displacement * *strength + **child_rb.vel * -dampening);
                *child_rb.ang_vel = parent_rb.ang_vel;
                *child_trans.rot = parent_trans.rot * **offset_rot;
            }
        }
    }
}
