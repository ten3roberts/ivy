use crate::systems::{
    apply_effectors_system, gravity_system, integrate_angular_velocity_system,
    integrate_velocity_system,
};
use flax::World;
use glam::Vec3;
use ivy_assets::AssetCache;
use ivy_collision::{
    check_collisions_system, collisions_tree_gizmos_system, components::collision_tree,
    register_system, update_system, BoundingBox, BvhNode, CollisionTree,
};
use ivy_core::{
    engine, gravity,
    update_layer::{FixedTimeStep, Plugin},
};

pub struct PhysicsPlugin {
    gravity: Vec3,
    enable_gizmos: bool,
}

impl PhysicsPlugin {
    pub fn new() -> Self {
        Self {
            gravity: Vec3::ZERO,
            enable_gizmos: false,
        }
    }

    /// Set the gravity
    pub fn with_gravity(mut self, gravity: Vec3) -> Self {
        self.gravity = gravity;
        self
    }

    /// Enable physics gizmos
    pub fn with_gizmos(mut self, enable_gizmos: bool) -> Self {
        self.enable_gizmos = enable_gizmos;
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
        world.set(engine(), gravity(), self.gravity)?;
        world.set(
            engine(),
            collision_tree(),
            CollisionTree::new(BvhNode::new(BoundingBox::new(Vec3::ONE * 1.0, Vec3::ZERO))),
        )?;

        let dt = time_step.delta_time() as f32;

        schedule
            .with_system(gravity_system())
            .with_system(integrate_velocity_system(dt))
            .with_system(integrate_angular_velocity_system(dt))
            .with_system(apply_effectors_system(dt))
            .with_system(register_system())
            .with_system(update_system())
            .with_system(check_collisions_system());

        if self.enable_gizmos {
            schedule.with_system(collisions_tree_gizmos_system());
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
