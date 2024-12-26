use std::time::Duration;

use flax::{system, FetchExt, World};
use ivy_assets::AssetCache;
use ivy_core::{
    components::{delta_time, engine},
    update_layer::{Plugin, ScheduleSetBuilder},
};

use crate::components::animator;

use super::player::Animator;

pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn install(
        &self,
        _: &mut World,
        _: &AssetCache,
        schedules: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        schedules
            .per_tick_mut()
            .with_system(animation_step_system());
        Ok(())
    }
}

#[system(args(dt=delta_time().source(engine()).copied()))]
pub fn animation_step(animator: &mut Animator, dt: Duration) {
    animator.step(dt.as_secs_f32());
}
