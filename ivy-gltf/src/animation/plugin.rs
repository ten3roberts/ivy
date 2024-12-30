use std::time::Duration;

use flax::{components::child_of, system, FetchExt, World};
use glam::{Quat, Vec3};
use ivy_assets::{Asset, AssetCache};
use ivy_core::{
    components::{delta_time, engine, position, rotation},
    update_layer::{Plugin, ScheduleSetBuilder},
};

use crate::components::{animator, skin, track_bone};

use super::{player::Animator, skin::Skin};

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
            .with_system(animation_step_system())
            .with_system(follow_bone_plugin_system());

        Ok(())
    }
}

#[system(args(dt=delta_time().source(engine()).copied()))]
fn animation_step(animator: &mut Animator, dt: Duration) {
    animator.step(dt.as_secs_f32());
}

#[system(args(animator=(animator(), skin()).traverse(child_of)))]
fn follow_bone_plugin(
    position: &mut Vec3,
    rotation: &mut Quat,
    animator: (&Animator, &Asset<Skin>),
    track_bone: &String,
) {
    let (animator, skin) = animator;
    if let Some(joint) = skin
        .joints()
        .iter()
        .find(|v| v.name.as_ref() == Some(track_bone))
    {
        if let Some(joint_target) = animator.joint_targets().get(&joint.scene_index) {
            *position = joint_target.pos;
            *rotation = joint_target.rotation;
        }
    }
}
