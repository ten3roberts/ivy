use flax::Entity;
use glam::Mat4;
use slotmap::{new_key_type, SlotMap};

use crate::{PersistentContact, BoundingBox, Collider, NodeIndex, NodeState};

pub type ContactMap = SlotMap<ContactIndex, PersistentContact>;
pub type BodyMap = SlotMap<BodyIndex, Body>;

new_key_type! {
    pub struct BodyIndex;
    pub struct ContactIndex;
}

#[derive(Debug, Clone)]
/// Data contained in each object in the tree.
///
/// Copied and retained from the ECS for easy access
pub struct Body {
    pub id: Entity,
    pub collider: Collider,
    pub bounds: BoundingBox,
    pub extended_bounds: BoundingBox,
    pub transform: Mat4,
    pub is_trigger: bool,
    pub state: NodeState,
    pub movable: bool,
    pub node: NodeIndex,

    // island links
    pub island: BodyIndex,
    pub next_body: BodyIndex,
    pub prev_body: BodyIndex,
}

impl ivy_core::gizmos::DrawGizmos for Body {
    fn draw_primitives(&self, gizmos: &mut ivy_core::gizmos::GizmosSection) {
        <BoundingBox as ivy_core::gizmos::DrawGizmos>::draw_primitives(
            &self.extended_bounds,
            gizmos,
        )
    }
}
