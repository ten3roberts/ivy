use ivy_base::math::Inverse;
use ivy_base::Position;
use ivy_collision::Contact;
use ultraviolet::Vec3;

use crate::components::Resitution;
use crate::{bundles::*, util::point_vel};

/// Generates an impulse for solving a collision.
pub fn resolve_collision(
    intersection: &Contact,
    a: &RbQuery,
    a_pos: Position,
    b: &RbQuery,
    b_pos: Position,
) -> Vec3 {
    let ra = intersection.points[0] - a_pos;
    let rb = intersection.points[1] - b_pos;
    let aw = *a.ang_vel;
    let bw = *b.ang_vel;
    let n = intersection.normal;

    let a_vel = **a.vel + point_vel(ra, aw);
    let b_vel = **b.vel + point_vel(rb, bw);
    let contact_rel = (a_vel - b_vel).dot(n);

    let resitution = a.resitution.min(b.resitution.0);

    if contact_rel < 0.0 {
        // eprintln!("Separating");
        return Vec3::zero();
    }
    let j = -(1.0 + resitution) * contact_rel * (a.mass.inv() + b.mass.inv()).inv()
        + ra.cross(n).mag_sq() * a.ang_mass.inv()
        + rb.cross(n).mag_sq() * b.ang_mass.inv();

    let impulse = j * intersection.normal;
    impulse
}

pub fn resolve_static_collision(
    contact: Position,
    normal: Vec3,
    a_resitution: Resitution,
    b: &RbQuery,
    b_pos: Position,
) -> Vec3 {
    let rb = contact - b_pos;
    let bw = *b.ang_vel;
    let n = normal;

    let b_vel = **b.vel + point_vel(rb, bw);
    let contact_rel = (-b_vel).dot(n);

    let resitution = a_resitution.min(b.resitution.0);

    if contact_rel < 0.0 {
        // eprintln!("Separating");
        return Vec3::zero();
    }
    let j = -(1.0 + resitution) * contact_rel * **b.mass + rb.cross(n).mag_sq() * b.ang_mass.inv();

    let impulse = j * normal;
    impulse
}
