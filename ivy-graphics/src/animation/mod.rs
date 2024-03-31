use glam::{Quat, Vec3};
use gltf::animation::util::{ReadOutputs, Rotations, Scales, Translations};

mod animations;
mod animator;
pub use animations::*;
pub use animator::*;

use crate::Skin;

#[derive(Debug)]
/// Represents an animation with a name
pub struct Animation {
    name: String,
    skin: usize,
    duration: f32,
    channels: Vec<Channel>,
}

impl Animation {
    pub fn from_gltf(
        animation: gltf::Animation,
        skins: &[Skin],
        buffers: &[gltf::buffer::Data],
    ) -> Vec<Self> {
        let mut animations: Vec<Self> = Vec::new();

        animation.channels().for_each(|channel| {
            let target = channel.target();

            let joint = target.node().index();

            let skin = match skins.iter().position(|skin| skin.joint(joint).is_some()) {
                Some(val) => val,
                None => return,
            };

            let reader = channel.reader(|buffer| Some(&buffers[buffer.index()]));
            let inputs = reader.read_inputs().unwrap();
            let outputs = reader.read_outputs().unwrap();

            let values = KeyFrameValues::new(outputs);
            let times = inputs.collect();

            let channel = Channel {
                joint,
                times,
                values,
            };

            if let Some(animation) = animations.iter_mut().find(|val| val.skin == skin) {
                if let Some(duration) = channel.times.last() {
                    animation.duration = animation.duration.max(*duration);
                }
                animation.channels.push(channel)
            } else {
                animations.push(Self {
                    name: animation.name().unwrap_or_default().to_string(),
                    skin,
                    duration: channel.times.last().cloned().unwrap_or_default(),
                    channels: vec![channel],
                })
            }
        });

        animations
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

    /// Get the animation's skin.
    pub fn skin(&self) -> usize {
        self.skin
    }

    /// Get a reference to the animation's name.
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
}

type ChannelIndex = usize;
type Frame = usize;

#[derive(Debug, Clone)]
pub enum KeyFrameValues {
    Positions(Vec<Vec3>),
    Rotations(Vec<Quat>),
    Scales(Vec<Vec3>),
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

#[derive(Debug, Clone)]
/// A channel describes a single transform component of a bone
pub struct Channel {
    /// Joint index
    joint: usize,
    times: Vec<f32>,
    values: KeyFrameValues,
}
