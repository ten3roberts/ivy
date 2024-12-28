use std::{
    collections::{BTreeMap, BTreeSet},
    path::PathBuf,
};

use anyhow::Context;
use glam::{Mat4, Quat};
use gltf::buffer;
use itertools::Itertools;
use ivy_assets::{Asset, AssetCache, AsyncAssetDesc};
use ivy_core::components::TransformBundle;

use crate::Document;

use super::{Animation, Channel, KeyFrameValues};

pub type JointIndex = usize;

#[derive(Debug)]
pub struct Joint {
    pub name: Option<String>,
    pub scene_index: usize,
    /// Transform vertex to bone space when no animation is applied
    pub inverse_bind_matrix: Mat4,
    pub(crate) local_bind_transform: TransformBundle,
    pub children: Vec<JointIndex>,
}

pub struct Skin {
    // Map from node index to index
    joint_map: BTreeMap<JointIndex, usize>,
    joints: Vec<Joint>,
    animations: Vec<Asset<Animation>>,
    roots: BTreeSet<JointIndex>,
}

impl Skin {
    pub fn load_from_document(
        assets: &AssetCache,
        document: &gltf::Document,
        buffer_data: &[buffer::Data],
    ) -> anyhow::Result<Vec<Asset<Self>>> {
        // NOTE: each joint in a skin refers to a node in the scene hierarchy
        let joint_maps = document
            .skins()
            .map(|v| {
                v.joints()
                    .enumerate()
                    .map(|(i, v)| (v.index(), i))
                    .collect::<BTreeMap<_, _>>()
            })
            .collect_vec();

        let mut skin_animations = BTreeMap::<usize, Vec<Animation>>::new();

        for animation in document.animations() {
            animation.channels().for_each(|channel| {
                let target = channel.target();

                let joint_scene_index = target.node().index();

                let Some(skin_index) = joint_maps
                    .iter()
                    .position(|v| v.contains_key(&joint_scene_index))
                else {
                    tracing::error!("Missing skin for animation");
                    return;
                };

                let reader = channel.reader(|buffer| Some(&buffer_data[buffer.index()]));

                let inputs = reader.read_inputs().unwrap();
                let outputs = reader.read_outputs().unwrap();

                let values = KeyFrameValues::new(outputs);
                let times = inputs.collect();

                let channel = Channel {
                    joint_scene_index,
                    times,
                    values,
                };

                let skin_animations = skin_animations.entry(skin_index).or_default();

                match skin_animations.last_mut() {
                    Some(v) => v.channels.push(channel),
                    None => {
                        skin_animations.push(Animation {
                            label: animation.name().unwrap_or("unknown").to_string().into(),
                            channels: vec![channel],
                        });
                    }
                }
            });
        }

        document
            .skins()
            .enumerate()
            .map(|(i, skin)| {
                let reader = skin.reader(|buffer| Some(&buffer_data[buffer.index()]));
                let joints = skin
                    .joints()
                    .zip(reader.read_inverse_bind_matrices().unwrap())
                    .map(|(joint, ibm)| {
                        let transform = joint.transform().decomposed();

                        Joint {
                            scene_index: joint.index(),
                            inverse_bind_matrix: Mat4::from_cols(
                                ibm[0].into(),
                                ibm[1].into(),
                                ibm[2].into(),
                                ibm[3].into(),
                            ),

                            local_bind_transform: TransformBundle {
                                pos: transform.0.into(),
                                rotation: Quat::from_array(transform.1),
                                scale: transform.2.into(),
                            },
                            children: joint.children().map(|val| val.index()).collect(),
                            name: joint.name().map(|v| v.to_string()),
                        }
                    })
                    .collect_vec();

                assert_eq!(skin.index(), i);
                let animations = skin_animations
                    .remove(&skin.index())
                    .unwrap_or_default()
                    .into_iter()
                    .map(|v| assets.insert(v))
                    .collect();

                let joint_map = joint_maps[i].clone();
                let mut roots = joints
                    .iter()
                    .map(|v| v.scene_index)
                    .collect::<BTreeSet<_>>();

                for joint in &joints {
                    let node = document.nodes().nth(joint.scene_index).unwrap();
                    tracing::info!(
                        index = joint.scene_index,
                        children = ?node.children().map(|v| v.index()).collect_vec(),
                        "node"
                    );

                    for child in node.children() {
                        roots.remove(&child.index());
                    }
                }

                tracing::info!(?roots, "roots");

                // let armature = document
                //     .nodes()
                //     .find(|node| node.name() == Some(name))
                //     .ok_or(Error::MissingArmature)?;

                // // Find the intersect of armature children and joints

                // let roots = armature
                //     .children()
                //     .filter(|val| joint_map.contains_key(&val.index()))
                //     .map(|val| val.index())
                //     .collect();
                Ok(assets.insert(Self {
                    joints,
                    joint_map,
                    animations,
                    roots,
                }))
            })
            .try_collect()
    }

    /// Transform a node index to a joint index used for meshes
    pub fn joint_to_index(&self, index: JointIndex) -> usize {
        self.joint_map[&index]
    }

    // pub fn joint_count(&self) -> usize {
    //     self.joints.len()
    // }

    pub fn find_joint_from_node_index(&self, index: JointIndex) -> Option<&Joint> {
        self.joints.get(*self.joint_map.get(&index)?)
    }

    pub fn joints(&self) -> &[Joint] {
        &self.joints
    }

    pub fn roots(&self) -> &BTreeSet<JointIndex> {
        &self.roots
    }

    pub fn animations(&self) -> &[Asset<Animation>] {
        &self.animations
    }
}

#[derive(
    Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
pub struct SkinDesc {
    document: PathBuf,
    node: String,
}

impl AsyncAssetDesc for SkinDesc {
    type Output = Skin;
    type Error = anyhow::Error;

    async fn create(&self, assets: &AssetCache) -> Result<Asset<Skin>, Self::Error> {
        let document: Asset<Document> = assets.from_path(&self.document).await?;

        let skin = document
            .find_node(&self.node)
            .with_context(|| {
                format!(
                    "Node {:?} not found in document {:?}",
                    self.node, self.document
                )
            })?
            .skin()
            .context("Missing skin")?;

        Ok(skin)
    }
}
