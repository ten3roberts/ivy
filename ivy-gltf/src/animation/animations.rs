use std::borrow::Cow;

use glam::{Quat, Vec3};
use gltf::{
    animation::util::{ReadOutputs, Rotations, Scales, Translations},
    buffer,
};
use itertools::Itertools;
use ivy_assets::{fs::AssetPath, Asset, AssetCache, AsyncAssetDesc, AsyncAssetExt};
use ordered_float::OrderedFloat;

use crate::Document;

pub struct Animation {
    label: Cow<'static, str>,
    channels: Vec<Channel>,
}

impl Animation {
    pub fn new(animation: &gltf::Animation, buffer_data: &[buffer::Data]) -> anyhow::Result<Self> {
        let channels = animation
            .channels()
            .map(|channel| {
                let target = channel.target();

                let joint_scene_index = target.node().index();

                let reader = channel.reader(|buffer| Some(&buffer_data[buffer.index()]));

                let inputs = reader.read_inputs().unwrap();
                let outputs = reader.read_outputs().unwrap();

                let values = KeyFrameValues::new(outputs);
                let times = inputs.collect();

                Channel {
                    joint_scene_index,
                    times,
                    values,
                }
            })
            .collect_vec();

        Ok(Animation {
            label: animation.name().unwrap_or("unknown").to_string().into(),
            channels,
        })
    }

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
    pub(crate) joint_scene_index: usize,
    pub(crate) times: Vec<f32>,
    pub(crate) values: KeyFrameValues,
}

impl Channel {
    pub fn duration(&self) -> Option<f32> {
        self.times.last().copied()
    }
}

#[derive(Debug)]
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct AnimationDesc {
    pub document: AssetPath<Document>,
    pub animation: String,
}

impl AsyncAssetDesc for AnimationDesc {
    type Output = Animation;
    type Error = anyhow::Error;

    async fn create(&self, assets: &AssetCache) -> Result<Asset<Animation>, Self::Error> {
        let document: Asset<Document> = self.document.load_async(assets).await?;

        let gltf_document = document.data().gltf();
        let Some(animation) = gltf_document
            .animations()
            .find(|v| v.name() == Some(&self.animation))
        else {
            anyhow::bail!(
                "Animation {:?} not found in document {:?}",
                self.animation,
                self.document
            )
        };

        Ok(assets.insert(Animation::new(&animation, document.data().buffer_data())?))
    }
}
