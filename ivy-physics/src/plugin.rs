use crate::{
    components::physics_state,
    state::{PhysicsState, PhysicsStateConfiguration},
    systems::{
        apply_effectors_system, contact_gizmos_system, dampening_system, generate_contacts_system,
        gizmo_system, gravity_system, integrate_angular_velocity_system, integrate_velocity_system,
        island_graph_gizmo_system, register_bodies_system, solve_contacts_system,
        sync_simulation_bodies_system, unregister_bodies_system, update_bodies_system,
        update_simulation_bodies_system,
    },
};
use flax::World;
use glam::Vec3;
use ivy_assets::AssetCache;
use ivy_core::{
    components::{engine, gravity},
    update_layer::{FixedTimeStep, Plugin},
};

#[derive(Default)]
pub struct GizmoSettings {
    pub bvh_tree: bool,
    pub contacts: bool,
    pub island_graph: bool,
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
            gravity: Vec3::ZERO,
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

impl Plugin<FixedTimeStep> for PhysicsPlugin {
    fn install(
        &self,
        world: &mut World,
        _: &AssetCache,
        schedule: &mut flax::ScheduleBuilder,
        time_step: &FixedTimeStep,
    ) -> anyhow::Result<()> {
        let dt = time_step.delta_time() as f32;

        world.set(engine(), gravity(), self.gravity)?;
        world.set(
            engine(),
            physics_state(),
            PhysicsState::new(&self.configuration, dt),
        )?;

        schedule
            .with_system(integrate_velocity_system(dt))
            .with_system(integrate_angular_velocity_system(dt))
            .with_system(gravity_system())
            .with_system(dampening_system(dt))
            .with_system(apply_effectors_system(dt))
            .with_system(register_bodies_system())
            .with_system(unregister_bodies_system(world))
            .with_system(update_bodies_system());

        if self.gizmos.rigidbody {
            schedule.with_system(gizmo_system(dt));
        }

        schedule.with_system(update_simulation_bodies_system());

        schedule
            .with_system(generate_contacts_system())
            .with_system(solve_contacts_system())
            .with_system(sync_simulation_bodies_system());

        if self.gizmos.bvh_tree {
            // schedule.with_system(collisions_tree_gizmos_system());
        }

        if self.gizmos.contacts {
            schedule.with_system(contact_gizmos_system());
        }

        if self.gizmos.island_graph {
            schedule.with_system(island_graph_gizmo_system());
        }

        Ok(())
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
