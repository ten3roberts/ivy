use std::collections::{BTreeMap, BTreeSet};

use anyhow::Context;
use glam::{Mat4, Quat};
use gltf::buffer;
use itertools::Itertools;
use ivy_assets::{fs::AssetPath, Asset, AssetCache, AsyncAssetDesc, AsyncAssetExt};
use ivy_core::components::TransformBundle;

use crate::Document;

use super::player::Animator;

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

        document
            .skins()
            .enumerate()
            .map(|(i, skin)| {
                let reader = skin.reader(|buffer| Some(&buffer_data[buffer.index()]));
                let skin_joints = skin
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

                let mut roots = skin_joints
                    .iter()
                    .map(|v| v.scene_index)
                    .collect::<BTreeSet<_>>();

                for joint in &skin_joints {
                    let node = document.nodes().nth(joint.scene_index).unwrap();

                    for child in node.children() {
                        roots.remove(&child.index());
                    }
                }

                Ok(assets.insert(Self {
                    joints: skin_joints,
                    joint_map: joint_maps[i].clone(),
                    roots,
                }))
            })
            .try_collect()
    }

    pub fn update_skinning_matrix(&self, animator: &Animator, skinning_matrix: &mut Vec<Mat4>) {
        skinning_matrix.clear();
        skinning_matrix.resize(self.joints.len(), Mat4::IDENTITY);

        for &root in self.roots() {
            let index = self.joint_to_index(root);
            self.fill_buffer_recursive(animator, Mat4::IDENTITY, index, skinning_matrix);
        }
    }

    fn fill_buffer_recursive(
        &self,
        animator: &Animator,
        parent_transform: Mat4,
        joint_index: usize,
        buffer: &mut [Mat4],
    ) {
        let joint = &self.joints()[joint_index];
        let target = animator
            .joint_targets()
            .get(&joint.scene_index)
            .unwrap_or(&joint.local_bind_transform);

        let transform = parent_transform * target.to_mat4();
        buffer[joint_index] = transform * joint.inverse_bind_matrix;

        for &child in &joint.children {
            self.fill_buffer_recursive(animator, transform, self.joint_to_index(child), buffer);
        }
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
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct SkinDesc {
    document: AssetPath<Document>,
    node: String,
}

impl AsyncAssetDesc for SkinDesc {
    type Output = Skin;
    type Error = anyhow::Error;

    async fn create(&self, assets: &AssetCache) -> Result<Asset<Skin>, Self::Error> {
        let document: Asset<Document> = self.document.load_async(assets).await?;

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
