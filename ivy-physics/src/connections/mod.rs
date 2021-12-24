use derive_for::*;
use derive_more::*;
use hecs::Bundle;
use ivy_base::{Position, TransformBundle, TransformQueryMut};

mod systems;
pub use systems::*;
use ultraviolet::{Bivec3, Rotor3, Vec3};

use crate::{bundles::*, components::*, util::point_vel, Effector};

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
    pub struct PositionOffset(pub Vec3);
    pub struct RotationOffset(pub Rotor3);
);

impl PositionOffset {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }
}

impl RotationOffset {
    pub fn euler_angles(roll: f32, pitch: f32, yaw: f32) -> Self {
        Self(Rotor3::from_euler_angles(roll, pitch, yaw))
    }
    /// Creates an angular velocity from an axis angle rotation. Note: Axis is
    /// assumed to be normalized.
    pub fn axis_angle(angle: f32, axis: Vec3) -> Self {
        Self(Rotor3::from_angle_plane(
            angle,
            Bivec3::from_normalized_axis(axis),
        ))
    }
}

/// Marker type for two physically connected objects.
pub struct Connection;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionKind {
    /// Connection will not budge
    Rigid,
    /// The connection will excert a force to return to the desired position
    Spring { strength: f32, dampening: f32 },
}

impl Default for ConnectionKind {
    fn default() -> Self {
        Self::Rigid
    }
}

impl ConnectionKind {
    pub fn rigid() -> Self {
        Self::Rigid
    }

    pub fn spring(strength: f32, dampening: f32) -> Self {
        Self::Spring {
            strength,
            dampening,
        }
    }
}

impl ConnectionKind {
    fn update(
        &self,
        offset_pos: &PositionOffset,
        offset_rot: &RotationOffset,
        child_trans: TransformQueryMut,
        rb: RbQueryMut,
        parent_trans: &TransformBundle,
        parent_rb: &mut RbBundle,
        effector: &mut Effector,
    ) {
        // The desired postion
        let pos = Position(parent_trans.into_matrix().transform_point3(**offset_pos));
        let displacement = pos - *child_trans.pos;
        match self {
            Self::Rigid => {
                // The desired velocity
                let vel =
                    Velocity(point_vel(pos - parent_trans.pos, parent_rb.ang_vel) + *parent_rb.vel);

                let total_mass = *rb.mass + parent_rb.mass;

                let vel_diff = vel - *rb.vel;

                *rb.ang_vel = parent_rb.ang_vel;
                // *child_trans.rot = parent_trans.rot * **offset_rot;

                let child_inf = **rb.mass / *total_mass;
                let parent_inf = *parent_rb.mass / *total_mass;

                effector.translate(*displacement * parent_inf);
                parent_rb.effector.translate(-*displacement * child_inf);

                effector.apply_velocity_change(*vel_diff * parent_inf);
                parent_rb
                    .effector
                    .apply_velocity_change(*-vel_diff * child_inf);

                *child_trans.rot = parent_trans.rot * **offset_rot;
            }
            Self::Spring {
                strength,
                dampening,
            } => {
                let force = *displacement * *strength + **rb.vel * -dampening;
                effector.apply_force(force);
                parent_rb.effector.apply_force(-force);

                *rb.ang_vel = parent_rb.ang_vel;
                *child_trans.rot = parent_trans.rot * **offset_rot;
            }
        }
    }
}

#[derive(Default, Bundle, Clone, Copy, Debug, PartialEq)]
pub struct ConnectionBundle {
    pub kind: ConnectionKind,
    pub offset: PositionOffset,
    pub rotation_offset: RotationOffset,
}

impl ConnectionBundle {
    pub fn new(
        kind: ConnectionKind,
        offset: PositionOffset,
        rotation_offset: RotationOffset,
    ) -> Self {
        Self {
            kind,
            offset,
            rotation_offset,
        }
    }
}
