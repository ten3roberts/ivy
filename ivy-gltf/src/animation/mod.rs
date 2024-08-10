pub mod player;
pub mod skin;

use std::borrow::Cow;

use glam::{Quat, Vec3};
use gltf::animation::util::{ReadOutputs, Rotations, Scales, Translations};
use ordered_float::OrderedFloat;

pub struct Animation {
    label: Cow<'static, str>,
    channels: Vec<Channel>,
}

impl Animation {
    // /// Get a reference to the animation's duration.
    pub fn duration(&self) -> f32 {
        self.channels
            .iter()
            .filter_map(|v| v.duration())
            .max_by_key(|v| OrderedFloat(*v))
            .unwrap_or(0.0)
    }

    /// Get a reference to the animation's channels.
    pub fn channels(&self) -> &[Channel] {
        self.channels.as_ref()
    }

    pub fn label(&self) -> &str {
        &self.label
    }
}

pub struct Channel {
    joint_scene_index: usize,
    times: Vec<f32>,
    values: KeyFrameValues,
}

impl Channel {
    pub fn duration(&self) -> Option<f32> {
        self.times.last().copied()
    }
}

pub(crate) enum KeyFrameValues {
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
        Self::Positions(outputs.map(|output| output.into()).collect())
    }

    pub fn new_rot(outputs: Rotations) -> Self {
        Self::Rotations(outputs.into_f32().map(Quat::from_array).collect())
    }

    pub fn new_scale(outputs: Scales) -> Self {
        Self::Scales(outputs.map(|output| output.into()).collect())
    }
}
