use flax::Debuggable;
use glam::Mat4;

use crate::{body::BodyIndex, Collider};

flax::component! {
    pub collider: Collider => [ Debuggable ],
    pub collider_offset: Mat4 => [ Debuggable ],
    pub body_index: BodyIndex => [ Debuggable ],
}
