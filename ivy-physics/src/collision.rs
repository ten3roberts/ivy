use glam::Vec3;
use ivy_base::{
    components::Resitution, math::Inverse, AngularMass, AngularVelocity, Friction, Mass, Position,
    Velocity,
};
use ivy_collision::Contact;

use crate::util::point_vel;

#[derive(Debug, Clone, Default)]
pub(crate) struct ResolveObject {
    pub pos: Position,
    pub vel: Velocity,
    pub ang_vel: AngularVelocity,
    pub resitution: Resitution,
    pub mass: Mass,
    pub ang_mass: AngularMass,
    pub friction: Friction,
}

/// Generates an impulse for solving a collision.
pub(crate) fn resolve_collision(
    intersection: &Contact,
    a: &ResolveObject,
    b: &ResolveObject,
) -> Vec3 {
    let ra = intersection.points[0] - a.pos;
    let rb = intersection.points[1] - b.pos;
    let aw = a.ang_vel;
    let bw = b.ang_vel;
    let n = intersection.normal;

    let a_vel = a.vel + point_vel(ra, aw);
    let b_vel = b.vel + point_vel(rb, bw);
    let contact_rel = (a_vel - b_vel).dot(n);

    let resitution = a.resitution.min(*b.resitution);

    if contact_rel < 0.01 {
        // eprintln!("Separating");
        return Vec3::ZERO;
    }

    let j = -(1.0 + resitution) * contact_rel * (a.mass.inv() + b.mass.inv()).inv()
        + ra.cross(n).length_squared() * a.ang_mass.inv()
        + rb.cross(n).length_squared() * b.ang_mass.inv();

    let friction =
        a.friction.min(*b.friction) * j * (a_vel - b_vel).reject_from(n).normalize_or_zero();

    j * 0.9 * n + friction
}
