use flax::Debuggable;
use glam::Mat4;

use crate::{Collider, CollisionTree, BodyIndex};

flax::component! {
    pub collider: Collider => [ Debuggable ],
    pub collider_offset: Mat4 => [ Debuggable ],
    pub tree_index: BodyIndex => [ Debuggable ],
    pub collision_tree: CollisionTree,
}
