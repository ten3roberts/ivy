use flax::Debuggable;
use glam::{Mat4, Vec3};

use crate::{Collider, ObjectIndex};

flax::component! {
    pub collider: Collider => [ Debuggable ],
    pub collider_offset: Mat4 => [ Debuggable ],
    pub tree_index: ObjectIndex => [ Debuggable ],
}