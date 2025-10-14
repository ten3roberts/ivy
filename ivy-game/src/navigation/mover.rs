use std::time::Duration;

use flax::{component, system, FetchExt};
use glam::Vec3;
use ivy_assets::{stored::DynamicStore, AssetCache, Resource};
use ivy_core::{
    components::{delta_time, engine},
    update_layer::{Plugin, ScheduleSetBuilder},
    Bundle,
};
use ivy_physics::{
    components::{effector, velocity},
    Effector,
};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MovementConfiguration {
    pub max_speed: f32,
    pub max_acceleration: f32,
    pub deceleration: f32,
    pub constraint: MovementConstraint,
    pub movement_mode: MovementMode,
    #[serde(default)]
    pub kinematic: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MovementConstraint {
    None,
    Axis(Vec3),
    Plane(Vec3),
}

impl MovementConstraint {
    pub fn constrain(&self, v: Vec3) -> Vec3 {
        match self {
            MovementConstraint::None => v,
            MovementConstraint::Axis(axis) => v.project_onto(*axis),
            MovementConstraint::Plane(plane) => v.reject_from(plane.normalize_or_zero()),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MovementMode {
    // Acceleration is proportional to the velocity error (PD controller)
    ProportionalFalloff,
    // Acceleration is constant
    ConstantAcceleration,
}

impl MovementMode {
    pub fn get_acceleration(
        &self,
        max_acceleration: f32,
        target_speed: Vec3,
        current_speed: Vec3,
        max_speed: f32,
        dt: f32,
    ) -> Vec3 {
        match self {
            MovementMode::ProportionalFalloff => {
                let diff = target_speed - current_speed;
                diff.normalize_or_zero() * max_acceleration * (diff.length() / max_speed) * dt
            }
            MovementMode::ConstantAcceleration => {
                let diff = target_speed - current_speed;
                (max_acceleration * dt).min(diff.length()) * diff.normalize_or_zero()
            }
        }
    }
}

component! {
    movement_controller: MovementController,
    pub movement_direction: Vec3,
    pub movement_configuration: MovementConfiguration,
}

/// Allows smoothly accelerating the entity in the specified direction.
///
/// Acceleration is proportional to the remaining speed to the target speed.
///
/// This works similar to how an engine piston or a stride accelerates, where the force given
/// (assuming constant actuator speed) is proportional to the difference between the ground speed
/// and the actuator speed.
///
/// In result, this provides a natural acceleration and deceleration behavior.
pub struct MovementController {}

impl MovementController {
    #[system(require_all, args(delta_time=delta_time().source(engine()), movement_direction=movement_direction().copied()))]
    fn update_movement_system(
        self: &MovementController,
        movement_configuration: &MovementConfiguration,
        movement_direction: Vec3,
        velocity: &mut Vec3,
        effector: &mut Effector,
        delta_time: &Duration,
    ) {
        let dt = delta_time.as_secs_f32();

        let constrained_velocity = movement_configuration.constraint.constrain(*velocity);

        let acceleration = movement_configuration.movement_mode.get_acceleration(
            movement_configuration.max_acceleration,
            movement_configuration.constraint.constrain(
                movement_direction.clamp_length_max(1.0) * movement_configuration.max_speed,
            ),
            constrained_velocity,
            movement_configuration.max_speed,
            dt,
        );

        if movement_configuration.kinematic {
            *velocity += acceleration;
        } else {
            effector.apply_velocity_change(acceleration, true);
        }
    }
}

#[derive(Clone, Debug, Resource, Bundle, serde::Serialize, serde::Deserialize)]
pub struct MoverBundle {
    conf: MovementConfiguration,
}

impl Bundle for MoverBundle {
    fn mount(&self, entity: &mut flax::EntityBuilder) {
        entity
            .set(movement_controller(), MovementController {})
            .set(movement_configuration(), self.conf)
            .set_default(movement_direction());
    }
}

pub struct MoverPlugin;

impl Plugin for MoverPlugin {
    fn install(
        &self,
        _: &mut flax::World,
        _: &AssetCache,
        _: &mut DynamicStore,
        schedules: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        schedules
            .fixed_mut()
            .with_system(MovementController::update_movement_system());

        Ok(())
    }
}
