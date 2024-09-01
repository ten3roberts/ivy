use flax::{error::MissingComponent, EntityRef};
use glam::Vec3;
use ivy_collision::{contact::ContactSurface, Intersection};
use ivy_core::{
    angular_mass, angular_velocity, friction, mass, math::Inverse, palette::convert, restitution,
    velocity, world_transform,
};

use crate::util::velocity_at_point;

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
pub(crate) fn calculate_impulse_response(
    a: &ResolveObject,
    b: &ResolveObject,
    normal: Vec3,
    point: Vec3,
) -> Vec3 {
    let to_a = point - a.pos;
    let to_b = point - b.pos;

    let a_w = a.ang_vel;
    let b_w = b.ang_vel;

    assert!(normal.is_normalized());
    let normal = normal.normalize();

    let a_vel = a.vel + a_w.cross(to_a);
    let b_vel = b.vel + b_w.cross(to_b);

    let contact_velocity = (b_vel - a_vel).dot(normal);

    let restitution = a.resitution.min(b.resitution);

    // objects are separating
    if contact_velocity >= 0.0 {
        return Vec3::ZERO;
    }

    let inverse_inertia = 1.0 / a.mass + 1.0 / b.mass;

    let a_inertia_tensor = 1.0 / a.ang_mass * to_a.cross(normal).cross(to_a);
    let b_inertia_tensor = 1.0 / b.ang_mass * to_b.cross(normal).cross(to_b);

    let inverse_inertia_tensor = (a_inertia_tensor + b_inertia_tensor).dot(normal);

    let num = -(1.0 + restitution) * contact_velocity;
    let denom: f32 = inverse_inertia + inverse_inertia_tensor;

    // assert!(denom.is_normal());
    let impulse = num / denom;

    let friction = a.friction.min(b.friction)
        * impulse
        * (a_vel - b_vel).reject_from(normal).normalize_or_zero();

    // assert!(impulse > 0.0);
    return impulse * normal + friction;

    // let angular_impulse = to_a.cross(normal).length_squared() * 1.0 / a.ang_mass ;

    // let j = -(1.0 + restitution) * contact_rel * (a.mass.recip() + b.mass.recip()).recip()
    //     + to_a.cross(normal).length_squared() * a.ang_mass.inv()
    //     + to_b.cross(normal).length_squared() * b.ang_mass.inv();

    // j * normal + friction
    // return linear_impulse * 0.99;
}
