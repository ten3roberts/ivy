use glam::{Mat4, Quat, Vec3};
use ivy_core::components::{ConnectionKind, TransformQueryMutItem};

mod systems;
pub use systems::*;

/// Updates a connection that has no rigidbody
pub fn update_fixed(_: Vec3, offset_rot: Quat, parent: Mat4, child: &mut TransformQueryMutItem) {
    let (_, parent_rot, parent_pos) = parent.to_scale_rotation_translation();
    *child.pos = parent_pos;
    *child.rotation = parent_rot * offset_rot;
}
