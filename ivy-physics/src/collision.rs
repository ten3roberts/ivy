use hecs::Entity;
use ivy_collision::Intersection;
use ultraviolet::Vec3;

use crate::components::*;

/// Collision event
#[derive(Debug, Clone, Copy)]
pub struct Collision {
    pub a: Entity,
    pub b: Entity,
    pub intersection: Intersection,
}

pub fn point_vel(p: Vec3, w: AngularVelocity) -> Vec3 {
    if w.mag_sq() < std::f32::EPSILON {
        Vec3::default()
    } else {
        -p.cross(*w)
    }
}

/// Generates an impulse for solving a collision.
pub fn resolve_collision(intersection: Intersection, a: &RbQuery, b: &RbQuery) -> Vec3 {
    let ra = intersection.points[0] - **a.pos;
    let rb = intersection.points[1] - **b.pos;
    let aw = a.ang_vel.cloned().unwrap_or_default();
    let bw = b.ang_vel.cloned().unwrap_or_default();
    let n = intersection.normal;

    let a_vel = *a.vel.cloned().unwrap_or_default() + point_vel(ra, aw);
    let b_vel = *b.vel.cloned().unwrap_or_default() + point_vel(rb, bw);
    let contact_rel = (a_vel - b_vel).dot(n);

    eprintln!(
        "aw: {:?}, bw: {:?}, a_vel: {:?}, b_vel: {:?}, perp: {:?}, {:?}",
        aw,
        bw,
        a_vel,
        b_vel,
        (intersection.points[1] - **b.pos).cross(*bw),
        contact_rel,
    );

    let resitution = a.resitution.min(b.resitution.0);

    if contact_rel < 0.0 {
        // eprintln!("Separating");
        return Vec3::zero();
    }

    let max_mass = Mass(std::f32::MAX);
    let max_ang_mass = AngularMass(std::f32::MAX);

    let a_ang_mass = **a.ang_mass.unwrap_or(&max_ang_mass);
    let b_ang_mass = **b.ang_mass.unwrap_or(&max_ang_mass);

    let a_mass = **a.mass.unwrap_or(&max_mass);
    let b_mass = **b.mass.unwrap_or(&max_mass);

    let j = -(1.0 + resitution) * contact_rel
        / (1.0 / a_mass
            + 1.0 / b_mass
            + ra.cross(n).mag_sq() / a_ang_mass
            + rb.cross(n).mag_sq() / b_ang_mass);

    // eprintln!("Mass: {:?}", intersection.normal);
    let impulse = j * intersection.normal;
    dbg!(impulse);
    impulse
}
