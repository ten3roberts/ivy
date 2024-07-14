use std::collections::BTreeMap;

use glam::{Mat4, Quat, Vec3};
use itertools::Itertools;
use ivy_assets::Asset;
use ivy_core::{palette::chromatic_adaptation::TransformMatrix, TransformBundle};

use super::{
    skin::{Joint, Skin},
    Animation,
};

pub struct Animator {
    joint_targets: BTreeMap<usize, TransformBundle>,
    player: AnimationPlayer,
}

impl Animator {
    pub fn new(animation: Asset<Animation>) -> Self {
        Self {
            joint_targets: BTreeMap::new(),
            player: AnimationPlayer::new(animation),
        }
    }

    pub fn step(&mut self, step_time: f32) {
        self.joint_targets.clear();

        self.player.step(step_time, |joint, target_value| {
            let joint_target = self.joint_targets.entry(joint).or_insert(TransformBundle {
                pos: Vec3::ZERO,
                rotation: Quat::IDENTITY,
                scale: Vec3::ONE,
            });

            match target_value {
                AnimationTarget::Position(v) => joint_target.pos = v,
                AnimationTarget::Rotation(v) => joint_target.rotation = v,
                AnimationTarget::Scale(v) => joint_target.scale = v,
            }
        });
    }

    pub fn fill_buffer(&self, skin: &Asset<Skin>, buffer: &mut [Mat4]) {
        for &root in skin.roots() {
            let index = skin.joint_to_index(root);
            self.fill_buffer_recursive(skin, Mat4::IDENTITY, index, buffer);
        }
    }

    pub fn fill_buffer_recursive(
        &self,
        skin: &Asset<Skin>,
        parent_transform: Mat4,
        index: usize,
        buffer: &mut [Mat4],
    ) {
        let joint = &skin.joints()[index];
        let target = self
            .joint_targets
            .get(&joint.scene_index)
            .unwrap_or(&joint.local_bind_transform);

        let transform = parent_transform * target.to_mat4();
        buffer[index] = transform * joint.inverse_bind_matrix;

        for &child in &joint.children {
            self.fill_buffer_recursive(skin, transform, skin.joint_to_index(child), buffer);
        }
    }
}

pub struct AnimationPlayer {
    progress: f32,
    animation: Asset<Animation>,
    channels: Vec<ChannelState>,
}

impl AnimationPlayer {
    pub fn new(animation: Asset<Animation>) -> Self {
        Self {
            progress: 0.0,
            channels: animation
                .channels()
                .iter()
                .map(|_| ChannelState { left_keyframe: 0 })
                .collect_vec(),
            animation,
        }
    }

    pub fn step(&mut self, step_time: f32, mut writer: impl FnMut(usize, AnimationTarget)) {
        self.progress += step_time;

        if self.progress > self.animation.duration() {
            self.channels.iter_mut().for_each(|v| v.left_keyframe = 0);
            self.progress %= self.animation.duration();
        }

        for (i, state) in self.channels.iter_mut().enumerate() {
            let channel = &self.animation.channels()[i];

            let mut right_keyframe = state.left_keyframe + 1;
            let mut right_keyframe_time = channel.times[right_keyframe];

            assert!(self.progress >= channel.times[state.left_keyframe]);

            // tracing::info!(state.left_keyframe, right_keyframe_time, self.progress);

            while self.progress > right_keyframe_time {
                if state.left_keyframe + 1 == channel.times.len() - 1 {
                    state.left_keyframe = 0;
                    right_keyframe = 1;
                    right_keyframe_time = channel.times[1];
                    break;
                } else {
                    state.left_keyframe += 1;

                    right_keyframe = state.left_keyframe + 1;
                    right_keyframe_time = channel.times[right_keyframe];
                }
            }

            let left_keyframe_time = channel.times[state.left_keyframe];

            let t =
                (self.progress - left_keyframe_time) / (right_keyframe_time - left_keyframe_time);

            match &channel.values {
                super::KeyFrameValues::Positions(v) => {
                    let v = v[state.left_keyframe].lerp(v[right_keyframe], t);
                    writer(channel.joint_scene_index, AnimationTarget::Position(v));
                }
                super::KeyFrameValues::Rotations(v) => {
                    let v = v[state.left_keyframe].lerp(v[right_keyframe], t);
                    writer(channel.joint_scene_index, AnimationTarget::Rotation(v));
                }
                super::KeyFrameValues::Scales(v) => {
                    let v = v[state.left_keyframe].lerp(v[right_keyframe], t);
                    writer(channel.joint_scene_index, AnimationTarget::Scale(v));
                }
            };
        }
    }
}

pub enum AnimationTarget {
    Position(Vec3),
    Rotation(Quat),
    Scale(Vec3),
}

struct ChannelState {
    left_keyframe: usize,
}
