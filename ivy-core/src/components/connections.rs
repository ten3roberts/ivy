use flax::Debuggable;
use glam::{Quat, Vec3};

flax::component! {
    pub position_offset: Vec3 => [ Debuggable ],
    pub rotation_offset: Quat => [ Debuggable ],
    pub connection(id): ConnectionKind => [ Debuggable ],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConnectionKind {
    /// Connection will not budge
    Rigid,
    /// The connection will exert a force to return to the desired position
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
