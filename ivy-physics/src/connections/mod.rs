use glam::{Mat4, Quat, Vec3};
use ivy_base::{ConnectionKind, TransformQueryMut, TransformQueryMutItem};

mod systems;
pub use systems::*;

use crate::{bundles::*, util::point_vel, Effector};

/// Updates a connection that has no rigidbody
pub fn update_fixed(
    offset_pos: Vec3,
    offset_rot: Quat,
    parent: Mat4,
    child: &mut TransformQueryMutItem,
) {
    let (_, parent_rot, parent_pos) = parent.to_scale_rotation_translation();
    *child.pos = parent_pos;
    *child.rotation = parent_rot * offset_rot;
}

pub fn apply_connection_constraints(
    kind: &ConnectionKind,
    offset_pos: Vec3,
    offset_rot: Quat,
    child_trans: TransformQueryMutItem,
    rb: RbQueryMutItem,
    parent: Mat4,
    parent_rb: &mut RbBundle,
    effector: &mut Effector,
    parent_effector: &mut Effector,
) {
    // The desired position
    let pos = parent.transform_point3(offset_pos);
    let displacement = pos - *child_trans.pos;

    let (_, parent_rot, parent_pos) = parent.to_scale_rotation_translation();

    match kind {
        ConnectionKind::Rigid => {
            // The desired velocity
            let vel = point_vel(pos - parent_pos, parent_rb.ang_vel) + parent_rb.vel;

            let total_mass = *rb.mass + parent_rb.mass;

            let vel_diff = vel - *rb.vel;

            *rb.ang_vel = parent_rb.ang_vel;
            // *child_trans.rot = parent_trans.rot * **offset_rot;

            let child_inf = *rb.mass / total_mass;
            let parent_inf = parent_rb.mass / total_mass;

            effector.translate(displacement * parent_inf);
            parent_effector.translate(-displacement * child_inf);

            effector.apply_velocity_change(vel_diff * parent_inf, true);
            parent_effector.apply_velocity_change(-vel_diff * child_inf, true);

            *child_trans.rotation = parent_rot * offset_rot;
        }
        ConnectionKind::Spring {
            strength,
            dampening,
        } => {
            let force = displacement * *strength + *rb.vel * -dampening;
            effector.apply_force(force, true);
            parent_effector.apply_force(-force, true);

            *rb.ang_vel = parent_rb.ang_vel;
            *child_trans.rotation = parent_rot * offset_rot;
        }
    }
}
