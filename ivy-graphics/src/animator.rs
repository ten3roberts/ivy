use std::collections::BTreeMap;

use gltf::animation::{
    util::{ReadOutputs, Rotations, Scales, Translations},
    Sampler,
};
use ivy_base::{Position, Rotation, Scale, TransformBundle};
use ivy_resources::{Handle, ResourceCache};
use ordered_float::OrderedFloat;
use ultraviolet::{Lerp, Rotor3, Slerp, Vec3};

use crate::Result;

#[derive(Debug)]
pub struct Animation {
    channels: Vec<Channel>,
}

impl Animation {
    pub fn from_gltf(animation: gltf::Animation, buffers: &[gltf::buffer::Data]) -> Self {
        let channels = animation.channels();

        let channels = channels
            .map(|channel| {
                let target = channel.target();
                let bone = target.node().index();
                let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
                let inputs = reader.read_inputs().unwrap();
                let outputs = reader.read_outputs().unwrap();

                let values = KeyFrameValues::new(outputs);
                let times = inputs.map(OrderedFloat).collect();

                Channel {
                    bone,
                    times,
                    values,
                }
            })
            .collect();

        Self { channels }
    }
}

#[derive(Debug, Clone)]
pub struct Animator {
    animation: Handle<Animation>,
    /// The keyframe index for each channel
    states: BTreeMap<usize, (usize, TransformBundle)>,
    /// The current animation time
    time: OrderedFloat<f32>,
}

impl Animator {
    pub fn new(animation: Handle<Animation>) -> Self {
        Self {
            animation,
            states: Default::default(),
            time: Default::default(),
        }
    }

    /// Moves the animators state forward by `dt`
    pub fn progress(&mut self, animations: &ResourceCache<Animation>, dt: f32) -> Result<()> {
        let animation = animations.get(self.animation)?;

        // Loop through all states and check if the frame should be changed

        self.time = OrderedFloat(dt);

        animation
            .channels
            .iter()
            .enumerate()
            .for_each(|(index, channel)| {
                // Get or initiate state
                let (current, transform) = self.states.entry(index).or_default();
                let next = (*current + 1) % channel.times.len();

                let start = channel.times[*current];
                let end = channel.times[next];

                let progress = (self.time - start) / (end - start);

                // Go to the next frame
                if progress >= OrderedFloat(1.0) {
                    *current = next;
                }

                match &channel.values {
                    KeyFrameValues::Positions(val) => {
                        transform.pos = val[*current].lerp(*val[next], *progress).into()
                    }
                    KeyFrameValues::Rotations(val) => {
                        transform.rot = val[*current].slerp(*val[next], *progress).into()
                    }
                    KeyFrameValues::Scales(val) => {
                        transform.scale = val[*current].lerp(*val[next], *progress).into()
                    }
                };
            });

        Ok(())
    }
}

#[derive(Debug, Clone)]
/// A channel describes a single transform component of a bone
struct Channel {
    /// Bone index
    bone: usize,
    times: Vec<OrderedFloat<f32>>,
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
                .map(|output| Rotor3::from_quaternion_array(output).into())
                .collect(),
        )
    }

    pub fn new_scale(outputs: Scales) -> Self {
        Self::Scales(outputs.map(|output| Vec3::from(output).into()).collect())
    }
}
