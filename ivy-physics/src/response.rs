use core::f32;
use std::f32::consts::PI;

use flax::component::ComponentValue;
use flax::{error::MissingComponent, EntityRef};
use flax::{Component, Entity, EntityIds, World};
use glam::Vec3;
use ivy_collision::body::BodyIndex;
use ivy_collision::contact::ContactSurface;
use ivy_collision::Contact;
use ivy_core::components::{
    angular_velocity, friction, inertia_tensor, is_static, mass, position, restitution, velocity,
    world_transform,
};

#[derive(Debug, Clone, Copy)]
pub struct SolverConfiguration {
    allowed_penetration: f32,
    accumulate_impulses: bool,
    correction_factor: f32,
}

impl SolverConfiguration {
    pub fn new() -> Self {
        Self {
            allowed_penetration: 0.05,
            correction_factor: 0.1,
            accumulate_impulses: true,
        }
    }
}

impl Default for SolverConfiguration {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Solver {
    configuration: SolverConfiguration,
    bodies: slotmap::SecondaryMap<BodyIndex, SimulationBody>,
    dt: f32,
}

impl Solver {
    pub fn new(configuration: SolverConfiguration, dt: f32) -> Self {
        Self {
            configuration,
            dt,
            bodies: Default::default(),
        }
    }

    pub(crate) fn add_body(&mut self, index: BodyIndex, body: SimulationBody) {
        self.bodies.insert(index, body);
    }

    pub(crate) fn remove_body(&mut self, index: BodyIndex) {
        self.bodies.remove(index);
    }

    pub(crate) fn bodies(&self) -> &slotmap::SecondaryMap<BodyIndex, SimulationBody> {
        &self.bodies
    }

    pub(crate) fn bodies_mut(&mut self) -> &mut slotmap::SecondaryMap<BodyIndex, SimulationBody> {
        &mut self.bodies
    }

    pub fn solve_contact(&mut self, contact: &Contact) -> flax::error::Result<()> {
        assert_ne!(contact.a, contact.b);

        let [a_body, b_body] = self
            .bodies
            .get_disjoint_mut([contact.a.body, contact.b.body])
            .expect("bodies must be disjoint");

        // let a = self.bodies[contact.a.body];
        // let b = self.bodies[contact.b.body];

        // Ignore collisions between two immovable objects
        if a_body.inv_mass == 0.0 && b_body.inv_mass == 0.0 {
            tracing::warn!("ignoring collision between two immovable objects");
            return Ok(());
        }

        let inv_mass = a_body.inv_mass + b_body.inv_mass;
        let restitution = a_body.restitution * b_body.restitution;
        let u_coeff = a_body.friction * b_body.friction;

        let surface = &contact.surface;
        let normal = surface.normal();

        assert!(normal.is_normalized());

        let mut acc_impulse = 0.0;

        let v_bias = -self.configuration.correction_factor
            * self.dt.recip()
            * (contact.surface.depth() - self.configuration.allowed_penetration).max(0.0);

        // for point in surface.points().iter().copied().chain([surface.midpoint()]) {
        // for point in [surface.midpoint()] {
        for &point in surface.points() {
            let to_a = point - a_body.pos;
            let to_b = point - b_body.pos;

            let inv_i = a_body.inverse_inertia_tensor * to_a.cross(normal).cross(to_a)
                + b_body.inverse_inertia_tensor * to_b.cross(normal).cross(to_b);
            let inertia = inv_mass + inv_i.dot(normal);

            let a_pvel = a_body.vel + a_body.ang_vel.cross(to_a);
            let b_pvel = b_body.vel + b_body.ang_vel.cross(to_b);

            let contact_vel = (b_pvel - a_pvel).dot(normal);

            let mut impulse = -(1.0 + restitution) * (contact_vel) / inertia;

            if self.configuration.accumulate_impulses {
                let old_acc_impulse = acc_impulse;
                acc_impulse = (acc_impulse + impulse).max(0.0);
                impulse = acc_impulse - old_acc_impulse;
            } else {
                impulse = impulse.max(0.0);
            }

            // use impulse for as friction normal force
            let impulse = impulse * normal;

            // apply impulse to points
            a_body.apply_impulse_at(-impulse, -to_a);
            b_body.apply_impulse_at(impulse, -to_b);
        }

        apply_friction(a_body, b_body, surface, normal, u_coeff, acc_impulse);

        let dampening = dampen(a_body, b_body, contact.surface.normal(), self.dt);
        if a_body.mass.is_infinite() {
            b_body.vel += dampening.linear;
        } else if b_body.mass.is_infinite() {
            a_body.vel -= dampening.linear;
        } else {
            a_body.vel -= dampening.linear * (b_body.mass * inv_mass);
            b_body.vel += dampening.linear * (a_body.mass * inv_mass);
        }

        if a_body.inertia_tensor.is_infinite() {
            b_body.ang_vel += dampening.angular;
        } else if b_body.inertia_tensor.is_infinite() {
            a_body.ang_vel -= dampening.angular;
        } else {
            a_body.ang_vel -= dampening.angular * (b_body.inertia_tensor * inv_mass);
            b_body.ang_vel += dampening.angular * (a_body.inertia_tensor * inv_mass);
        }

        fn try_write<T: ComponentValue>(entity: &EntityRef, component: Component<T>, value: T) {
            if let Ok(mut val) = entity.get_mut(component) {
                *val = value;
            }
        }

        a_body.pos += v_bias * self.dt * a_body.inv_mass / inv_mass;
        b_body.pos -= v_bias * self.dt * b_body.inv_mass / inv_mass;

        Ok(())
    }
}

fn apply_friction(
    a_body: &mut SimulationBody,
    b_body: &mut SimulationBody,
    surface: &ContactSurface,
    normal: Vec3,
    u_coeff: f32,
    normal_force: f32,
) {
    let point = surface.midpoint();
    let to_a = point - a_body.pos;
    let to_b = point - b_body.pos;

    let a_pvel = a_body.vel + a_body.ang_vel.cross(to_a);
    let b_pvel = b_body.vel + b_body.ang_vel.cross(to_b);

    let inv_mass = a_body.inv_mass + b_body.inv_mass;
    let inv_i = a_body.inverse_inertia_tensor * to_a.cross(normal).cross(to_a)
        + b_body.inverse_inertia_tensor * to_b.cross(normal).cross(to_b);
    let inertia = inv_mass + inv_i.dot(normal);

    // apply friction and disc friction to the midpoint of the surface
    // DO NOT apply friction to individual contact points, as it interfers with disc friction
    let friction_force = u_coeff * normal_force;

    let friction_force =
        friction_force.min(((a_pvel - b_pvel).reject_from(normal).length()) / inertia);

    let tangent = normal
        .cross(a_pvel - b_pvel)
        .cross(normal)
        .normalize_or_zero();

    let impulse = friction_force * tangent;

    let torque_mag = 2.0 / 3.0 * normal_force * u_coeff * (surface.area() / PI).sqrt();

    let rel_angular = (a_body.ang_vel - b_body.ang_vel)
        .project_onto(normal)
        .normalize_or_zero();

    let torque = rel_angular * torque_mag;

    a_body.apply_impulse_at(-impulse, -to_a);
    b_body.apply_impulse_at(impulse, -to_b);

    a_body.apply_angular_impulse(-torque);
    b_body.apply_angular_impulse(torque);
}

#[derive(Debug)]
pub(crate) struct SimulationBody {
    pub id: Entity,
    pub pos: Vec3,
    pub vel: Vec3,
    pub ang_vel: Vec3,
    pub restitution: f32,
    pub mass: f32,
    pub inertia_tensor: f32,
    pub inverse_inertia_tensor: f32,
    pub inv_mass: f32,
    pub friction: f32,
}

impl SimulationBody {
    pub fn from_entity(entity: &EntityRef) -> Result<Self, MissingComponent> {
        let pos = entity.get(world_transform())?.transform_point3(Vec3::ZERO);
        let vel = entity.get_copy(velocity()).unwrap_or_default();
        let ang_vel = entity.get_copy(angular_velocity()).unwrap_or_default();
        let restitution = entity.get_copy(restitution()).unwrap_or_default();
        let friction = entity.get_copy(friction()).unwrap_or_default();

        if entity.has(is_static()) {
            let resolve_body = Self {
                id: entity.id(),
                pos,
                vel,
                ang_vel,
                restitution,
                friction,
                mass: f32::INFINITY,
                inertia_tensor: f32::INFINITY,
                inverse_inertia_tensor: 0.0,
                inv_mass: 0.0,
            };

            Ok(resolve_body)
        } else {
            let inertia_tensor = entity.get_copy(inertia_tensor())?;
            let mass = entity.get_copy(mass())?;

            let resolve_body = Self {
                id: entity.id(),
                pos,
                vel,
                ang_vel,
                restitution,
                mass,
                inertia_tensor,
                inverse_inertia_tensor: inertia_tensor.recip(),
                inv_mass: mass.recip(),
                friction,
            };

            Ok(resolve_body)
        }
    }

    fn apply_impulse_at(&mut self, impulse: Vec3, to_a: Vec3) {
        self.vel += impulse * self.inv_mass;
        self.ang_vel += impulse.cross(to_a) * self.inverse_inertia_tensor;
    }

    fn apply_angular_impulse(&mut self, torque: Vec3) {
        self.ang_vel += torque * self.inverse_inertia_tensor;
    }
}

struct Dampening {
    linear: Vec3,
    angular: Vec3,
}

fn dampen(a: &SimulationBody, b: &SimulationBody, normal: Vec3, dt: f32) -> Dampening {
    const DAMPEN_FACTOR: f32 = 0.1;
    const ANGULAR_DAMPEN_FACTOR: f32 = 0.1;

    let transverse_vel = (a.vel - b.vel).reject_from(normal);

    let transverse_w = (a.ang_vel - b.ang_vel).reject_from(normal);

    Dampening {
        linear: transverse_vel * (1.0 - (1.0 / (1.0 + dt * DAMPEN_FACTOR))),
        angular: transverse_w * (1.0 - (1.0 / (1.0 + dt * ANGULAR_DAMPEN_FACTOR))),
    }
}
