use std::{collections::BTreeMap, slice::Iter};

use glam::{Mat4, Quat, Vec3};
use gltf::buffer::Data;
use smallvec::SmallVec;

use crate::{Error, JointTarget, Result};

pub struct Joint {
    /// Transform vertex to bone space when no animation is applied
    pub inverse_bind_matrix: Mat4,
    pub local_bind_transform: JointTarget,
    pub children: SmallVec<[usize; 4]>,
}

pub type JointIndex = usize;

pub struct Skin {
    // Map from node index to index
    joint_map: BTreeMap<JointIndex, usize>,
    roots: Vec<JointIndex>,
    joints: Vec<Joint>,
}

impl Skin {
    pub fn from_gltf(
        document: &gltf::Document,
        skin: gltf::Skin,
        buffers: &[Data],
    ) -> Result<Self> {
        let reader = skin.reader(|buffer| Some(&buffers[buffer.index()]));

        let joints = skin
            .joints()
            .enumerate()
            .zip(reader.read_inverse_bind_matrices().unwrap())
            .map(|((idx, joint), ibm)| {
                let index = joint.index();
                let transform = joint.transform().decomposed();
                (
                    (index, idx),
                    Joint {
                        inverse_bind_matrix: Mat4::from_cols(
                            ibm[0].into(),
                            ibm[1].into(),
                            ibm[2].into(),
                            ibm[3].into(),
                        ),
                        // local_bind_transform: Mat4::from_scale_rotation_translation(
                        //     transform.2.into(),
                        //     Quat::from_array(transform.1),
                        //     transform.0.into(),
                        // ),
                        local_bind_transform: JointTarget {
                            position: Vec3::from(transform.0),
                            rotation: Quat::from_array(transform.1),
                            scale: Vec3::from(transform.2),
                        },
                        children: joint.children().map(|val| val.index()).collect(),
                    },
                )
            });

        let (joint_map, joints): (BTreeMap<_, _>, Vec<_>) = joints.unzip();

        let name = skin.name().ok_or(Error::MissingArmature)?;
        let armature = document
            .nodes()
            .find(|node| node.name() == Some(name))
            .ok_or(Error::MissingArmature)?;

        // Find the intersect of armature children and joints

        let roots = armature
            .children()
            .filter(|val| joint_map.contains_key(&val.index()))
            .map(|val| val.index())
            .collect();

        Ok(Self {
            joint_map,
            joints,
            roots,
        })
    }

    /// Transform a node index to a joint index used for meshes
    pub fn joint_to_index(&self, index: JointIndex) -> usize {
        self.joint_map[&index]
    }

    pub fn joint_count(&self) -> usize {
        self.joints.len()
    }

    pub fn joint(&self, index: JointIndex) -> Option<&Joint> {
        self.joints.get(*self.joint_map.get(&index)?)
    }

    pub fn joints(&self) -> Iter<Joint> {
        self.joints.iter()
    }

    /// Get a reference to the skin's root.
    pub fn roots(&self) -> &[JointIndex] {
        &self.roots
    }
}
