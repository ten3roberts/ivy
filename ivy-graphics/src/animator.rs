use std::collections::{btree_map::Iter, BTreeMap};

use glam::{Quat, Vec3};
use gltf::animation::util::{ReadOutputs, Rotations, Scales, Translations};
use hecs_schedule::{Read, SubWorld};
use itertools::Itertools;
use ivy_base::{DeltaTime, Position, Rotation, Scale, TransformBundle, TransformMatrix};
use ivy_resources::{Handle, ResourceCache, ResourceView};
use ordered_float::OrderedFloat;

use crate::{JointIndex, Result, Skin};

#[derive(Debug)]
pub struct Animation {
    name: String,
    duration: f32,
    channels: Vec<Channel>,
}

impl Animation {
    pub fn from_gltf(animation: gltf::Animation, buffers: &[gltf::buffer::Data]) -> Self {
        let channels = animation.channels();

        let channels = channels
            .map(|channel| {
                let target = channel.target();
                let joint = target.node().index();
                let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
                let inputs = reader.read_inputs().unwrap();
                let outputs = reader.read_outputs().unwrap();

                let values = KeyFrameValues::new(outputs);
                let times = inputs.collect();

                Channel {
                    joint,
                    times,
                    values,
                }
            })
            .collect_vec();

        let duration = *channels
            .iter()
            .flat_map(|val| val.times.get(val.times.len() - 1))
            .map(|val| OrderedFloat(*val))
            .max()
            .unwrap_or_default();

        Self {
            name: animation.name().unwrap_or_default().to_owned(),
            duration,
            channels,
        }
    }

    /// Get a reference to the animation's name.
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// Get a reference to the animation's duration.
    pub fn duration(&self) -> f32 {
        self.duration
    }

    /// Get a reference to the animation's channels.
    pub fn channels(&self) -> &[Channel] {
        self.channels.as_ref()
    }

    pub fn channel(&self, index: usize) -> &Channel {
        &self.channels[index]
    }
}

type ChannelIndex = usize;
type Frame = usize;

#[derive(Debug, Clone)]
pub struct Animator {
    animation: Handle<Animation>,
    /// The keyframe index for each channel
    states: BTreeMap<ChannelIndex, Frame>,
    joints: BTreeMap<JointIndex, TransformBundle>,
    /// The current animation time
    time: f32,
}

impl Animator {
    pub fn new(animation: Handle<Animation>) -> Self {
        Self {
            animation,
            states: Default::default(),
            joints: Default::default(),
            time: Default::default(),
        }
    }

    /// Moves the animators state forward by `dt`
    pub fn progress(&mut self, animations: &ResourceCache<Animation>, dt: f32) -> Result<()> {
        let animation = animations.get(self.animation)?;

        // Loop through all states and check if the frame should be changed
        self.time = self.time + dt;

        if self.time > animation.duration() {
            self.time = self.time % animation.duration();
            self.states.clear();
        }

        // Populate all bones
        animation
            .channels
            .iter()
            .enumerate()
            .for_each(|(index, channel)| {
                // Get or initiate state
                let current = self.states.entry(index).or_default();

                let transform = self.joints.entry(channel.joint).or_default();

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
                        transform.pos = val[*current].lerp(*val[next], progress).into()
                    }
                    KeyFrameValues::Rotations(val) => {
                        transform.rot = val[*current].slerp(*val[next], progress).into()
                    }
                    KeyFrameValues::Scales(val) => {
                        transform.scale = val[*current].lerp(*val[next], progress).into()
                    }
                };
            });

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
}

#[derive(Debug, Clone)]
/// A channel describes a single transform component of a bone
pub struct Channel {
    /// Joint index
    joint: usize,
    times: Vec<f32>,
    values: KeyFrameValues,
}

#[derive(Debug, Clone)]
pub enum KeyFrameValues {
    Positions(Vec<Position>),
    Rotations(Vec<Rotation>),
    Scales(Vec<Scale>),
}

impl KeyFrameValues {
    fn new(outputs: ReadOutputs) -> Self {
        match outputs {
            ReadOutputs::Translations(val) => Self::new_pos(val),
            ReadOutputs::Rotations(val) => Self::new_rot(val),
            ReadOutputs::Scales(val) => Self::new_scale(val),
            ReadOutputs::MorphTargetWeights(_) => unimplemented!(),
        }
    }

    pub fn new_pos(outputs: Translations) -> Self {
        Self::Positions(outputs.map(|output| Vec3::from(output).into()).collect())
    }

    pub fn new_rot(outputs: Rotations) -> Self {
        Self::Rotations(
            outputs
                .into_f32()
                .map(|output| Quat::from_array(output).into())
                .collect(),
        )
    }

    pub fn new_scale(outputs: Scales) -> Self {
        Self::Scales(outputs.map(|output| Vec3::from(output).into()).collect())
    }
}
