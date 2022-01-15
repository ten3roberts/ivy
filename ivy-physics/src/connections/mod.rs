use ivy_base::{
    ConnectionKind, Position, PositionOffset, RotationOffset, TransformBundle, TransformQueryMut,
};

mod systems;
pub use systems::*;

use crate::{bundles::*, util::point_vel, Effector};

/// Updates a connection that has no rigidbody
pub fn update_fixed(
    offset_pos: &PositionOffset,
    offset_rot: &RotationOffset,
    parent: &TransformBundle,
    child: TransformQueryMut,
) {
    let pos = Position(parent.into_matrix().transform_point3(**offset_pos));
    *child.pos = pos;
    *child.rot = parent.rot * **offset_rot;
}

pub fn update_connection(
    kind: &ConnectionKind,
    offset_pos: &PositionOffset,
    offset_rot: &RotationOffset,
    child_trans: TransformQueryMut,
    rb: RbQueryMut,
    parent: &TransformBundle,
    parent_rb: &mut RbBundle,
    effector: &mut Effector,
) {
    // The desired postion
    let pos = Position(parent.into_matrix().transform_point3(**offset_pos));
    let displacement = pos - *child_trans.pos;
    match kind {
        ConnectionKind::Rigid => {
            // The desired velocity
            let vel = point_vel(pos - parent.pos, parent_rb.ang_vel) + parent_rb.vel;

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

            *child_trans.rot = parent.rot * **offset_rot;
        }
        ConnectionKind::Spring {
            strength,
            dampening,
        } => {
            let force = *displacement * *strength + **rb.vel * -dampening;
            effector.apply_force(force);
            parent_rb.effector.apply_force(-force);

            *rb.ang_vel = parent_rb.ang_vel;
            *child_trans.rot = parent.rot * **offset_rot;
        }
    }
}
