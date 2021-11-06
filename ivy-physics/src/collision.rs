use ivy_collision::Contact;
use ultraviolet::Vec3;

use crate::components::*;

pub fn point_vel(p: Vec3, w: AngularVelocity) -> Vec3 {
    if w.mag_sq() < std::f32::EPSILON {
        Vec3::default()
    } else {
        -p.cross(*w)
    }
}

/// Generates an impulse for solving a collision.
pub fn resolve_collision(intersection: &Contact, a: &RbQuery, b: &RbQuery) -> Vec3 {
    let ra = intersection.points[0] - **a.pos;
    let rb = intersection.points[1] - **b.pos;
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
    let j = -(1.0 + resitution) * contact_rel
        / (1.0 / **a.mass
            + 1.0 / **b.mass
            + ra.cross(n).mag_sq() / **a.ang_mass
            + rb.cross(n).mag_sq() / **b.ang_mass);

    let impulse = j * intersection.normal;
    impulse
}
