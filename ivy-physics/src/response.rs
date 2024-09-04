use std::f32::consts::PI;

use crate::components::effector;
use crate::systems::{get_rigid_root, round_to_zero};
use crate::RbQuery;
use flax::{error::MissingComponent, EntityRef};
use flax::{FetchExt, World};
use glam::Vec3;
use ivy_collision::contact::ContactSurface;
use ivy_collision::CollisionTree;
use ivy_core::components::{
    angular_mass, angular_velocity, friction, mass, position, restitution, velocity,
    world_transform,
};

pub fn resolve_collisions(
    world: &World,
    collision_tree: &CollisionTree,
    dt: f32,
) -> anyhow::Result<()> {
    for collision in collision_tree.active_collisions() {
        let a = world.entity(collision.a.entity)?;
        let b = world.entity(collision.b.entity)?;

        // Ignore triggers
        if collision.a.is_trigger || collision.b.is_trigger {
            return Ok(());
        }
        // Check for static collision
        else if collision.a.state.is_static() {
            resolve_static(&a, &b, &collision.contact, 1.0, dt)?;
            continue;
        } else if collision.b.state.is_static() {
            resolve_static(&b, &a, &collision.contact, -1.0, dt)?;
            continue;
        } else if collision.a.state.is_static() && collision.b.state.is_static() {
            tracing::warn!("static-static collision detected, ignoring");
            continue;
        }

        assert_ne!(collision.a, collision.b);

        // Trace up to the root of the rigid connection before solving
        // collisions
        let a = get_rigid_root(&world.entity(*collision.a).unwrap());
        let b = get_rigid_root(&world.entity(*collision.b).unwrap());

        // Ignore collisions between two immovable objects
        // if !a_mass.is_normal() && !b_mass.is_normal() {
        //     tracing::warn!("ignoring collision between two immovable objects");
        //     return Ok(());
        // }

        // let mut a_query = world.try_query_one::<(RbQuery, &Position, &Effector)>(a)?;
        // let (a, pos, eff) = a_query.get().unwrap();

        // // Modify mass to include all children masses

        let a_object = ResolveObject::from_entity(&a)?;
        let b_object = ResolveObject::from_entity(&b)?;

        let total_mass = a_object.mass + b_object.mass;

        assert!(
            total_mass > 0.0,
            "mass of two colliding objects must not be both zero"
        );

        let response = calculate_impulse_response(
            &a_object,
            &b_object,
            collision.contact.normal(),
            collision.contact.midpoint(),
            collision.contact.area(),
        );

        let dir = collision.contact.normal() * collision.contact.depth();

        let linear = round_to_zero(response.linear);
        let angular = round_to_zero(response.angular);

        let dampening = dampen(&a_object, &b_object, collision.contact.normal());

        {
            let effector = &mut *a.get_mut(effector())?;
            effector.apply_impulse_at(-linear, collision.contact.midpoint() - a_object.pos, true);
            effector.apply_torque(-angular);
            effector.translate(-dir * (b_object.mass / total_mass));
            effector.apply_velocity_change(-dampening.linear, true);
            effector.apply_angular_velocity_change(-dampening.angular);
        }

        {
            let effector = &mut *b.get_mut(effector())?;
            effector.apply_impulse_at(linear, collision.contact.midpoint() - b_object.pos, true);
            effector.apply_torque(angular);
            effector.translate(dir * (a_object.mass / total_mass));

            effector.apply_velocity_change(dampening.linear, true);
            effector.apply_angular_velocity_change(dampening.angular);
        }
    }

    Ok(())
}

// Resolves collision against a static or immovable object
fn resolve_static(
    a: &EntityRef,
    b: &EntityRef,
    contact: &ContactSurface,
    polarity: f32,
    dt: f32,
) -> anyhow::Result<()> {
    let query = &(
        restitution().opt_or_default(),
        friction().opt_or_default(),
        position(),
    );

    let mut a = a.query(&query);
    let a = a.get().unwrap();

    let query = &(RbQuery::new(), position(), effector().as_mut());

    let mut b = b.query(query);
    let Some(b) = b.get() else { return Ok(()) };
    let b_effector = b.2;

    let dv = b_effector.net_velocity_change(dt);
    let dw = b_effector.net_angular_velocity_change(dt);

    let b = ResolveObject {
        pos: *b.1,
        vel: *b.0.vel + dv,
        ang_vel: *b.0.ang_vel + dw,
        resitution: *b.0.restitution,
        mass: *b.0.mass,
        ang_mass: *b.0.ang_mass,
        friction: *b.0.friction,
    };

    let a = ResolveObject {
        pos: *a.2,
        resitution: *a.0,
        friction: *a.1,
        ..Default::default()
    };

    let normal = (contact.normal() * polarity).normalize();

    // tracing::info!(polarity, "{contact:.1}");
    let Response {
        linear: impulse,
        angular: torque,
    } = calculate_impulse_response(
        &ResolveObject {
            mass: f32::INFINITY,
            ang_mass: f32::INFINITY,
            ..a
        },
        &b,
        normal,
        contact.midpoint(),
        contact.area(),
    );

    assert!(
        impulse.is_finite(),
        "impulse: {impulse}, midpoint: {}, normal: {}",
        contact.midpoint(),
        normal
    );

    let dampening = dampen(&a, &b, normal);

    b_effector.apply_impulse_at(round_to_zero(impulse), contact.midpoint() - b.pos, true);
    b_effector.apply_angular_impulse(round_to_zero(torque));

    b_effector.apply_velocity_change(dampening.linear, true);
    b_effector.apply_angular_velocity_change(dampening.angular);

    b_effector.translate(normal * (contact.depth() - 0.001).max(0.0));

    Ok(())
}

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

struct Response {
    linear: Vec3,
    angular: Vec3,
}

fn dampen(a: &ResolveObject, b: &ResolveObject, normal: Vec3) -> Response {
    const DAMPEN_FACTOR: f32 = 1e-3;
    const ANGULAR_DAMPEN_FACTOR: f32 = 1e-2;
    let transverse_vel = (a.vel - b.vel).reject_from(normal);

    let transverse_w = (a.ang_vel - b.ang_vel).reject_from(normal);

    Response {
        linear: transverse_vel * DAMPEN_FACTOR,
        angular: transverse_w * ANGULAR_DAMPEN_FACTOR,
    }
}

/// Generates an impulse for solving a collision.
fn calculate_impulse_response(
    a: &ResolveObject,
    b: &ResolveObject,
    normal: Vec3,
    point: Vec3,
    area: f32,
) -> Response {
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
        return Response {
            linear: Vec3::ZERO,
            angular: Vec3::ZERO,
        };
    }

    let inverse_inertia = 1.0 / a.mass + 1.0 / b.mass;

    let a_inertia_tensor = 1.0 / a.ang_mass * to_a.cross(normal).cross(to_a);
    let b_inertia_tensor = 1.0 / b.ang_mass * to_b.cross(normal).cross(to_b);

    let inverse_inertia_tensor = (a_inertia_tensor + b_inertia_tensor).dot(normal);

    let num = -(1.0 + restitution) * contact_velocity;
    let denom: f32 = inverse_inertia + inverse_inertia_tensor;

    // assert!(denom.is_normal());
    let impulse = num / denom;

    let u_coeff = a.friction.min(b.friction);
    let friction = u_coeff * impulse * (a_vel - b_vel).reject_from(normal).normalize_or_zero();

    let torque_mag = 2.0 / 3.0 * impulse * u_coeff * (area / PI).sqrt();

    let rel_angular = (a_w - b_w).project_onto(normal).normalize_or_zero();

    let disc_friction = rel_angular * torque_mag;

    // assert!(impulse > 0.0, "impulse: {impulse:?}");
    Response {
        linear: impulse * normal + friction,
        angular: disc_friction,
    }
}
