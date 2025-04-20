use std::time::Duration;

use flax::{components::child_of, system, FetchExt, World};
use glam::{Mat4, Quat, Vec3};
use itertools::Itertools;
use ivy_assets::{Asset, AssetCache};
use ivy_core::{
    components::{delta_time, engine, position, rotation},
    update_layer::{Plugin, ScheduleSetBuilder},
};

use crate::components::{animator, skin, skin_matrix, track_bone};

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
            .with_system(computer_skinning_system())
            .with_system(follow_bone_plugin_system());

        Ok(())
    }
}

#[system(args(dt=delta_time().source(engine()).copied()))]
fn animation_step_system(animator: &mut Animator, dt: Duration) {
    animator.step(dt.as_secs_f32());
}

#[system(args(animator=animator().traverse(child_of).modified()))]
fn computer_skinning_system(skin: &Skin, skin_matrix: &mut Vec<Mat4>, animator: &Animator) {
    skin.update_skinning_matrix(animator, skin_matrix);
}

#[system(args(skin=(skin(), skin_matrix()).traverse(child_of).expect()))]
fn follow_bone_plugin_system(
    position: &mut Vec3,
    rotation: &mut Quat,
    skin: (&Asset<Skin>, &Vec<Mat4>),
    track_bone: &String,
) -> anyhow::Result<()> {
    if let Some((joint_index, joint)) = skin
        .0
        .joints()
        .iter()
        .find_position(|v| v.name.as_ref() == Some(track_bone))
    {
        let target = skin.1[joint_index] * joint.inverse_bind_matrix.inverse();
        let (_, target_rot, target_pos) = target.to_scale_rotation_translation();

        *position = target_pos;
        *rotation = target_rot;
    } else {
        tracing::error!("Failed to find bone {track_bone:?}")
    }

    Ok(())
}
