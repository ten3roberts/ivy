use derive_for::*;
use derive_more::*;
use hecs::{Bundle, DynamicBundleClone};

use glam::{EulerRot, Quat, Vec3};

derive_for!(
    (
        Add,
        AsRef,
        Clone,
        Copy,
        Debug,
        Deref,
        DerefMut,
        Div,
        DivAssign,
        Sub,
        From,
        Into,
        Mul,
        MulAssign,
        Default,
        PartialEq,
        Display,
    );
    /// Describes the offset of the entity from the parent
    pub struct PositionOffset(pub Vec3);
    pub struct RotationOffset(pub Quat);
);

impl PositionOffset {
    pub fn new(x: f32, y: f32, z: f32) -> Self {
        Self(Vec3::new(x, y, z))
    }
}

impl RotationOffset {
    pub fn euler_angles(roll: f32, pitch: f32, yaw: f32) -> Self {
        Self(Quat::from_euler(EulerRot::ZXY, roll, pitch, yaw))
    }
    /// Creates an angular velocity from an axis angle rotation. Note: Axis is
    /// assumed to be normalized.
    pub fn axis_angle(axis: Vec3, angle: f32) -> Self {
        Self(Quat::from_axis_angle(axis, angle))
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

#[derive(Default, Bundle, Clone, Copy, Debug, PartialEq, DynamicBundleClone)]
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
