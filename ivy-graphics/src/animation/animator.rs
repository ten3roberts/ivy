use std::collections::{btree_map::Iter, BTreeMap};

use flax::{Query, World};
use glam::{Mat4, Quat, Vec3};
use ivy_assets::Asset;

use crate::{
    components::animator, Animation, AnimationStore, JointIndex, KeyFrameValues, Result, Skin,
};

use super::{ChannelIndex, Frame};

/// Information regarding a single animation's playback
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnimationInfo {
    pub influence: f32,
    pub speed: f32,
    pub repeat: bool,
}

impl AnimationInfo {
    pub fn new(influence: f32, speed: f32, repeat: bool) -> Self {
        Self {
            influence,
            speed,
            repeat,
        }
    }
}

impl Default for AnimationInfo {
    fn default() -> Self {
        Self {
            influence: 1.0,
            speed: 1.0,
            repeat: true,
        }
    }
}

/// Used for interpolating between keyframes
#[derive(Debug, Clone, Copy)]
pub struct JointTarget {
    pub(crate) position: Vec3,
    pub(crate) rotation: Quat,
    pub(crate) scale: Vec3,
}
impl JointTarget {
    fn as_matrix(&self) -> Mat4 {
        Mat4::from_scale_rotation_translation(self.scale, self.rotation, self.position)
    }
}

impl Default for JointTarget {
    fn default() -> Self {
        Self {
            position: Vec3::ZERO,
            rotation: Quat::IDENTITY,
            scale: Vec3::ONE,
        }
    }
}

/// Drives the animation of a skin for an entity
///
/// Allows playing multiple overlapping animations at once
#[derive(Debug, Clone)]
pub struct Animator {
    states: BTreeMap<Asset<Animation>, AnimationState>,
    /// The keyframe index for each channel
    joints: BTreeMap<JointIndex, (JointTarget, usize)>,
    generation: usize,

    /// The current animation time
    animations: AnimationStore,
}

impl Animator {
    pub fn new(animations: impl Into<AnimationStore>) -> Self {
        Self {
            states: Default::default(),
            joints: Default::default(),
            generation: 0,
            animations: animations.into(),
        }
    }

    /// Moves the animators state forward by `dt`
    pub fn progress(&mut self, dt: f32) -> Result<()> {
        self.generation += 1;
        let joints = &mut self.joints;
        let generation = self.generation;

        self.states
            .iter_mut()
            .try_for_each(|(_, animation)| animation.progress(dt, joints, generation))?;

        Ok(())
    }

    pub fn stop_animation(&mut self, animation: &str) -> Result<()> {
        let animation = self.animations.find(animation)?;

        self.stop_animation_handle(animation);
        Ok(())
    }

    pub fn stop_all(&mut self) {
        self.states.clear();
    }

    /// Play an animation by index
    pub fn stop_animation_index(&mut self, animation: usize) -> Result<()> {
        let animation = self.animations.get(animation)?;

        self.stop_animation_handle(animation);
        Ok(())
    }

    pub fn stop_animation_handle(&mut self, animation: Asset<Animation>) {
        self.states
            .entry(animation)
            .and_modify(|val| val.playing = false);
    }

    /// Play an animation/action by name
    pub fn play_animation(&mut self, animation: &str, info: AnimationInfo) -> Result<()> {
        let animation = self.animations.find(animation)?;

        self.play_animation_handle(animation, info);

        Ok(())
    }

    /// Play an animation by handle
    /// **NOTE**: Behaviour is undefined if an animation for a different skin is used
    fn play_animation_handle(&mut self, animation: Asset<Animation>, info: AnimationInfo) {
        self.states
            .entry(animation)
            .and_modify(|val| {
                val.reset();
                val.info = info
            })
            .or_insert_with_key(|animation| AnimationState::new(animation.clone(), info));
    }

    /// Play an animation by index
    pub fn play_animation_index(&mut self, animation: usize, info: AnimationInfo) -> Result<()> {
        let animation = self.animations.get(animation)?;

        self.play_animation_handle(animation, info);
        Ok(())
    }

    pub fn joints(&self) -> Iter<JointIndex, (JointTarget, usize)> {
        self.joints.iter()
    }

    pub fn fill_sparse(
        &mut self,
        skin: &Skin,
        data: &mut [Mat4],
        current: JointIndex,
        parent: Mat4,
    ) {
        let generation = self.generation;
        let (transform, _) = self.joints.entry(current).or_insert_with(|| {
            (
                skin.joint(current)
                    .expect("Missing joint in skin")
                    .local_bind_transform,
                generation,
            )
        });

        let skin_joint = skin.joint(current).expect("Missing joint in skin");
        let current_transform = parent * transform.as_matrix();
        // dbg!(current_transform);
        data[skin.joint_to_index(current)] = current_transform * skin_joint.inverse_bind_matrix;

        for child in skin_joint.children.iter() {
            self.fill_sparse(skin, data, *child, current_transform)
        }
    }

    /// ECS system for updating all animators
    pub fn system(world: &World, dt: f32) -> Result<()> {
        Query::new(animator().as_mut())
            .borrow(world)
            .iter()
            .try_for_each(|animator| animator.progress(dt))
    }

    /// Get a reference to the animator's animations.
    pub fn animations(&self) -> &AnimationStore {
        &self.animations
    }
}

#[derive(Debug, Clone, PartialEq)]
struct AnimationState {
    animation: Asset<Animation>,
    states: BTreeMap<ChannelIndex, Frame>,
    time: f32,
    playing: bool,
    info: AnimationInfo,
}

impl AnimationState {
    pub fn new(animation: Asset<Animation>, info: AnimationInfo) -> Self {
        Self {
            animation,
            states: BTreeMap::new(),
            playing: true,
            time: 0.0,
            info,
        }
    }

    /// Moves the state forward by `dt`
    pub fn progress(
        &mut self,
        dt: f32,
        joints: &mut BTreeMap<ChannelIndex, (JointTarget, usize)>,
        generation: usize,
    ) -> Result<()> {
        if !self.playing {
            return Ok(());
        }

        // Loop through all states and check if the frame should be changed
        self.time += dt * self.info.speed.abs();

        if self.time > self.animation.duration() {
            if self.info.repeat {
                self.time = self.time % self.animation.duration();
                self.states.clear();
            } else {
                self.time = self.animation.duration();
                self.playing = false;
            }
        }

        let dir = self.info.speed.signum() as isize;

        self.animation
            .channels
            .iter()
            .enumerate()
            .for_each(|(index, channel)| {
                // Get or initiate state
                let current = self.states.entry(index).or_default();

                let (transform, gen) = joints
                    .entry(channel.joint)
                    .or_insert_with(|| (JointTarget::default(), generation));

                if *gen != generation {
                    *transform = JointTarget::default();
                    *gen = generation;
                }

                let next = (*current + (channel.times.len() as isize + dir) as usize)
                    % channel.times.len();

                let start = channel.times[*current];
                let end = channel.times[next];

                let progress = if dir == -1 {
                    1.0 - ((self.time - start) / (end - start)).abs()
                } else {
                    (self.time - start) / (end - start)
                };

                // Go to the next frame
                if progress >= 1.0 {
                    *current = next;
                }

                match &channel.values {
                    KeyFrameValues::Positions(val) => {
                        transform.position +=
                            val[*current].lerp(val[next], progress) * self.info.influence
                    }
                    KeyFrameValues::Rotations(val) => {
                        transform.rotation = val[*current].slerp(val[next], progress)
                    }
                    KeyFrameValues::Scales(val) => {
                        transform.scale =
                            val[*current].lerp(val[next], progress) * self.info.influence
                    }
                };
            });

        Ok(())
    }

    fn reset(&mut self) {
        if !self.playing {
            self.playing = true;
            // self.time = 0.0;
        }
        if !self.info.repeat {
            self.time = 0.0;
            self.states.clear();
        }
    }
}
