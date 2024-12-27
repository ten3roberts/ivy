use std::collections::BTreeMap;

use glam::{Mat4, Quat, Vec3};
use itertools::Itertools;
use ivy_assets::{map::AssetMap, Asset};
use ivy_core::components::TransformBundle;

use super::{skin::Skin, Animation};

pub struct Animator {
    joint_targets: BTreeMap<usize, TransformBundle>,
    players: AssetMap<Animation, AnimationPlayer>,
}

impl Animator {
    pub fn new() -> Self {
        Self {
            joint_targets: BTreeMap::new(),
            players: Default::default(),
        }
    }

    pub fn step(&mut self, step_time: f32) {
        // self.joint_targets.clear();

        for (_, player) in &mut self.players {
            player.step(step_time, |joint, target_value| {
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
    }

    pub fn start_animation(&mut self, player: AnimationPlayer) {
        self.players.insert_with_id(player.animation.id(), player);
    }

    pub fn is_playing(&self, animation: &Asset<Animation>) -> bool {
        self.players.contains(animation)
    }

    pub fn get_playing_animation(
        &mut self,
        animation: &Asset<Animation>,
    ) -> Option<&mut AnimationPlayer> {
        self.players.get_mut(animation)
    }

    pub fn stop_animation(&mut self, animation: &Asset<Animation>) {
        self.players.remove(animation);
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
        joint_index: usize,
        buffer: &mut [Mat4],
    ) {
        let joint = &skin.joints()[joint_index];
        let target = self
            .joint_targets
            .get(&joint.scene_index)
            .unwrap_or(&joint.local_bind_transform);

        let transform = parent_transform * target.to_mat4();
        buffer[joint_index] = transform * joint.inverse_bind_matrix;

        for &child in &joint.children {
            self.fill_buffer_recursive(skin, transform, skin.joint_to_index(child), buffer);
        }
    }

    pub fn joint_targets(&self) -> &BTreeMap<usize, TransformBundle> {
        &self.joint_targets
    }
}

impl Default for Animator {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AnimationPlayer {
    progress: f32,
    speed: f32,
    looping: bool,
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
            speed: 1.0,
            looping: false,
        }
    }

    pub fn set_speed(&mut self, speed: f32) {
        self.speed = speed;
    }

    pub fn set_looping(&mut self, looping: bool) {
        self.looping = looping;
    }

    pub fn step(&mut self, step_time: f32, mut writer: impl FnMut(usize, AnimationTarget)) {
        let finished = (self.speed > 0.0 && self.progress > self.animation.duration())
            || (self.speed < 0.0 && self.progress < 0.0);

        // Ensure we are actually past the end in an already renderer state
        if !self.looping && finished {
            return;
        }

        self.progress += step_time * self.speed;

        // Do this after stepping to not show past-end lerps when we could have wrapped
        if self.looping && self.progress > self.animation.duration() {
            self.channels.iter_mut().for_each(|v| v.left_keyframe = 0);
            self.progress %= self.animation.duration();
        } else if self.looping && self.progress < 0.0 {
            self.channels
                .iter_mut()
                .zip(self.animation.channels())
                .for_each(|(v, channel)| v.left_keyframe = channel.times.len() - 2);
            self.progress = (self.progress + self.animation.duration()) % self.animation.duration();
        }

        for (i, state) in self.channels.iter_mut().enumerate() {
            let channel = &self.animation.channels()[i];
            let mut right_keyframe = state.left_keyframe + 1;
            // assert!(self.progress >= channel.times[state.left_keyframe]);

            if self.speed < 0.0 {
                while self.progress < channel.times[state.left_keyframe] {
                    if state.left_keyframe == 0 {
                        if self.looping {
                            state.left_keyframe = channel.times.len() - 2;
                            right_keyframe = state.left_keyframe + 1;
                        }
                        break;
                    } else {
                        state.left_keyframe -= 1;

                        right_keyframe = state.left_keyframe + 1;
                    }
                }
            } else {
                while self.progress > channel.times[right_keyframe] {
                    if state.left_keyframe + 1 == channel.times.len() - 1 {
                        if self.looping {
                            state.left_keyframe = 0;
                            right_keyframe = 1;
                        }
                        break;
                    } else {
                        state.left_keyframe += 1;

                        right_keyframe = state.left_keyframe + 1;
                    }
                }
            }

            let progress = self.progress.clamp(0.0, self.animation.duration());
            let t = (progress - channel.times[state.left_keyframe])
                / (channel.times[right_keyframe] - channel.times[state.left_keyframe]);

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

#[derive(Debug)]
pub enum AnimationTarget {
    Position(Vec3),
    Rotation(Quat),
    Scale(Vec3),
}

struct ChannelState {
    left_keyframe: usize,
}
