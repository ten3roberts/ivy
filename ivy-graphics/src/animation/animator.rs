use std::collections::{btree_map::Iter, BTreeMap};

use hecs_schedule::{Read, SubWorld};
use ivy_base::{DeltaTime, Position, Rotation, Scale, TransformBundle, TransformMatrix};
use ivy_resources::{Handle, ResourceCache, ResourceView};

use crate::{Animation, AnimationStore, JointIndex, KeyFrameValues, Result, Skin};

use super::{ChannelIndex, Frame};

#[derive(Debug, Clone)]
pub struct Animator {
    states: BTreeMap<Handle<Animation>, AnimationState>,
    /// The keyframe index for each channel
    joints: BTreeMap<JointIndex, TransformBundle>,
    /// The current animation time
    animations: AnimationStore,
}

impl Animator {
    pub fn new(animations: impl Into<AnimationStore>) -> Self {
        Self {
            states: Default::default(),
            joints: Default::default(),
            animations: animations.into(),
        }
    }

    /// Moves the animators state forward by `dt`
    pub fn progress(&mut self, animations: &ResourceCache<Animation>, dt: f32) -> Result<()> {
        let joints = &mut self.joints;

        joints.clear();

        self.states
            .iter_mut()
            .try_for_each(|(_, animation)| animation.progress(animations, dt, joints))?;

        Ok(())
    }

    /// Play an animation/action by name
    pub fn play_animation(&mut self, animation: &str, repeat: bool, influence: f32) -> Result<()> {
        let animation = self.animations.find(animation)?;

        self.play_animation_handle(animation, repeat, influence);

        Ok(())
    }

    /// Play an animation by handle
    /// **NOTE**: Behaviour is undefined if an animation for a different skin is used
    fn play_animation_handle(
        &mut self,
        animation: Handle<Animation>,
        repeat: bool,
        influence: f32,
    ) {
        self.states
            .entry(animation)
            .and_modify(|val| {
                val.reset();
                val.repeat = repeat
            })
            .or_insert_with(|| AnimationState::new(animation, repeat, influence));
    }

    /// Play an animation by index
    pub fn play_animation_index(
        &mut self,
        animation: usize,
        repeat: bool,
        influence: f32,
    ) -> Result<()> {
        let animation = self.animations.get(animation)?;

        self.play_animation_handle(animation, repeat, influence);
        Ok(())
    }

    pub fn joints(&self) -> Iter<JointIndex, TransformBundle> {
        self.joints.iter()
    }

    pub fn fill_sparse(
        &mut self,
        skin: &Skin,
        data: &mut [TransformMatrix],
        current: JointIndex,
        parent: TransformMatrix,
    ) {
        let transform = self.joints.entry(current).or_insert_with(|| {
            skin.joint(current)
                .expect("Missing joint in skin")
                .local_bind_transform
        });
        let skin_joint = skin.joint(current).expect("Missing joint in skin");
        let current_transform = parent * transform.into_matrix();
        // dbg!(current_transform);
        data[skin.joint_to_index(current)] = current_transform * skin_joint.inverse_bind_matrix;

        for child in skin_joint.children.iter() {
            self.fill_sparse(skin, data, *child, current_transform)
        }
    }

    /// ECS system for updating all animators
    pub fn system(
        world: SubWorld<&mut Self>,
        animations: ResourceView<Animation>,
        dt: Read<DeltaTime>,
    ) -> Result<()> {
        world
            .native_query()
            .iter()
            .try_for_each(|(_, animator)| animator.progress(&*animations, **dt))
    }

    /// Get a reference to the animator's animations.
    pub fn animations(&self) -> &AnimationStore {
        &self.animations
    }
}

#[derive(Default, Debug, Clone, PartialEq)]
struct AnimationState {
    animation: Handle<Animation>,
    states: BTreeMap<ChannelIndex, Frame>,
    repeat: bool,
    time: f32,
    playing: bool,
    influence: f32,
}

impl AnimationState {
    pub fn new(animation: Handle<Animation>, repeat: bool, influence: f32) -> Self {
        Self {
            animation,
            states: BTreeMap::new(),
            repeat,
            playing: true,
            time: 0.0,
            influence,
        }
    }

    /// Moves the state forward by `dt`
    pub fn progress(
        &mut self,
        animations: &ResourceCache<Animation>,
        dt: f32,
        joints: &mut BTreeMap<ChannelIndex, TransformBundle>,
    ) -> Result<()> {
        let animation = animations.get(self.animation)?;

        if !self.playing {
            return Ok(());
        }

        // Loop through all states and check if the frame should be changed
        self.time = self.time + dt;

        if self.time > animation.duration() {
            self.time = self.time % animation.duration();
            self.states.clear();
        }

        animation
            .channels
            .iter()
            .enumerate()
            .for_each(|(index, channel)| {
                // Get or initiate state
                let current = self.states.entry(index).or_default();

                let transform = joints
                    .entry(channel.joint)
                    .or_insert_with(|| TransformBundle {
                        pos: Position::zero(),
                        rot: Rotation::default(),
                        scale: Scale::zero(),
                    });

                let next = (*current + 1) % channel.times.len();

                let start = channel.times[*current];
                let end = channel.times[next];

                let progress = (self.time - start) / (end - start);

                // Go to the next frame
                if progress >= 1.0 {
                    *current = next;
                }

                match &channel.values {
                    KeyFrameValues::Positions(val) => {
                        transform.pos =
                            (val[*current].lerp(*val[next], progress) * self.influence).into()
                    }
                    KeyFrameValues::Rotations(val) => {
                        transform.rot = Rotation(val[*current].slerp(*val[next], progress))
                    }
                    KeyFrameValues::Scales(val) => {
                        transform.scale +=
                            (val[*current].lerp(*val[next], progress) * self.influence).into()
                    }
                };
            });
        Ok(())
    }

    fn reset(&mut self) {
        if !self.playing {
            self.playing = true;
            self.time = 0.0;
        }
    }
}
