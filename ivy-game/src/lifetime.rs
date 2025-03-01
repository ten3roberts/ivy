use std::time::Duration;

use flax::{component, components::child_of, system, CommandBuffer, Debuggable, Entity, FetchExt};
use ivy_core::{
    components::{delta_time, engine},
    update_layer::Plugin,
};

component! {
    pub lifetime: Lifetime => [Debuggable],
}

/// Despawns the entity after the set time
#[derive(Debug)]
pub struct Lifetime {
    remaining: Duration,
}

impl Lifetime {
    pub fn new(remaining: Duration) -> Self {
        Self { remaining }
    }

    #[system(args(dt=delta_time().source(engine())), with_cmd_mut)]
    fn update(self: &mut Lifetime, id: Entity, dt: &Duration, cmd: &mut CommandBuffer) {
        self.remaining = self.remaining.saturating_sub(*dt);

        if self.remaining == Duration::ZERO {
            cmd.despawn_recursive(child_of, id);
        }
    }
}

pub struct LifetimePlugin;

impl Plugin for LifetimePlugin {
    fn install(
        &self,
        _: &mut flax::World,
        _: &ivy_assets::AssetCache,
        schedules: &mut ivy_core::update_layer::ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        schedules.fixed_mut().with_system(Lifetime::update_system());

        Ok(())
    }
}
