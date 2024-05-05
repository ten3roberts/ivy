use flax::{error::MissingComponent, EntityRef};
use glam::Vec3;
use ivy_base::{
    angular_mass, angular_velocity, friction, mass, math::Inverse, restitution, velocity,
    world_transform,
};
use ivy_collision::Contact;

use crate::util::point_vel;

#[derive(Debug, Clone, Default)]
pub(crate) struct ResolveObject {
    pub pos: Vec3,
    pub vel: Vec3,
    pub ang_vel: Vec3,
    pub resitution: f32,
    pub mass: f32,
    pub ang_mass: f32,
    pub friction: f32,
}

impl ResolveObject {
    pub(crate) fn new(
        pos: Vec3,
        vel: Vec3,
        ang_vel: Vec3,
        resitution: f32,
        mass: f32,
        ang_mass: f32,
        friction: f32,
    ) -> Self {
        Self {
            pos,
            vel,
            ang_vel,
            resitution,
            mass,
            ang_mass,
            friction,
        }
    }

    pub fn from_entity(entity: &EntityRef) -> Result<Self, MissingComponent> {
        Ok(Self {
            pos: entity.get(world_transform())?.transform_point3(Vec3::ZERO),
            vel: entity.get_copy(velocity())?,
            ang_vel: entity.get_copy(angular_velocity())?,
            resitution: entity.get_copy(restitution())?,
            mass: entity.get_copy(mass())?,
            ang_mass: entity.get_copy(angular_mass())?,
            friction: entity.get_copy(friction())?,
        })
    }
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

    let resitution = a.resitution.min(b.resitution);

    if contact_rel < 0.01 {
        // eprintln!("Separating");
        return Vec3::ZERO;
    }

    let j = -(1.0 + resitution) * contact_rel * (a.mass.inv() + b.mass.inv()).inv()
        + ra.cross(n).length_squared() * a.ang_mass.inv()
        + rb.cross(n).length_squared() * b.ang_mass.inv();

    let friction =
        a.friction.min(b.friction) * j * (a_vel - b_vel).reject_from(n).normalize_or_zero();

    j * 0.99 * n + friction
}
