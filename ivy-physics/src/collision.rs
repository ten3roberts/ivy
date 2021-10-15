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

/// Generates an impulse for solving a collision.
pub fn resolve_collision(intersection: Intersection, a: RbQuery, b: RbQuery) -> Vec3 {
    dbg!(intersection.normal);
    let contact_rel = (*a.vel - *b.vel).dot(intersection.normal);
    let resitution = a.resitution.min(b.resitution.0);

    dbg!(contact_rel);
    if contact_rel < 0.0 {
        // eprintln!("Separating");
        return Vec3::zero();
    }

    let j = -(1.0 + resitution) * contact_rel / (1.0 / a.mass.0 + 1.0 / b.mass.0);
    // eprintln!("Mass: {:?}", intersection.normal);
    let impulse = j * intersection.normal;
    dbg!(impulse);
    impulse
}
