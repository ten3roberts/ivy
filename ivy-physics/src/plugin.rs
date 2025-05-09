use std::any::type_name;

use flax::World;
use glam::Vec3;
use ivy_assets::AssetCache;
use ivy_core::{
    components::engine,
    transforms::TransformUpdatePlugin,
    update_layer::{Plugin, ScheduleSetBuilder},
};

use crate::{
    components::{gravity, physics_state},
    state::{PhysicsState, PhysicsStateConfiguration},
    systems::{
        attach_joints_system, gizmo_system, register_bodies_system, unregister_bodies_system,
        unregister_colliders_system,
    },
};

#[derive(Default)]
pub struct GizmoSettings {
    pub rigidbody: bool,
}

pub struct PhysicsPlugin {
    gravity: Vec3,
    gizmos: GizmoSettings,
    configuration: PhysicsStateConfiguration,
}

impl PhysicsPlugin {
    pub fn new() -> Self {
        Self {
            gravity: -Vec3::Y * 9.81,
            gizmos: Default::default(),
            configuration: PhysicsStateConfiguration::default(),
        }
    }

    /// Set the gravity
    pub fn with_gravity(mut self, gravity: Vec3) -> Self {
        self.gravity = gravity;
        self
    }

    /// Enable physics gizmos
    pub fn with_gizmos(mut self, gizmos: GizmoSettings) -> Self {
        self.gizmos = gizmos;
        self
    }
}

impl Default for PhysicsPlugin {
    fn default() -> Self {
        Self::new()
    }
}

impl Plugin for PhysicsPlugin {
    fn install(
        &self,
        world: &mut World,
        _: &AssetCache,
        schedules: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        let dt = schedules.fixed_mut().time_step().delta_time() as f32;

        world.set(engine(), gravity(), self.gravity)?;
        world.set(
            engine(),
            physics_state(),
            PhysicsState::new(&self.configuration, dt),
        )?;

        let schedule = &mut *schedules.fixed_mut();
        schedule
            .with_system(unregister_bodies_system(world))
            .with_system(unregister_colliders_system(world))
            .with_system(register_bodies_system())
            .flush()
            .with_system(PhysicsState::register_colliders_system())
            .with_system(attach_joints_system(world))
            .flush()
            .with_system(PhysicsState::update_collider_position_system())
            .with_system(PhysicsState::update_body_data_system())
            .with_system(PhysicsState::apply_effectors_system())
            .with_system(PhysicsState::step_system())
            .with_system(PhysicsState::sync_bodies_after_step_system())
            .with_system(PhysicsState::process_events_system());

        if self.gizmos.rigidbody {
            schedule.with_system(gizmo_system());
        }

        Ok(())
    }

    fn after(&self) -> Vec<&str> {
        vec![type_name::<TransformUpdatePlugin>()]
    }
}

#[derive(Default, Debug, Clone)]
pub struct PhysicsLayerDesc {
    pub gravity: Vec3,
    pub debug: bool,
}

impl PhysicsLayerDesc {
    pub fn new(gravity: Vec3, debug: bool) -> Self {
        Self { gravity, debug }
    }
}
