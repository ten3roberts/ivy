use std::{collections::BTreeMap, slice::Iter};

use gltf::buffer::Data;
use ivy_base::{Position, Rotation, Scale, TransformBundle, TransformMatrix};
use smallvec::SmallVec;
use ultraviolet::{Mat4, Rotor3, Vec3};

use crate::{Error, Result};

pub struct Joint {
    /// Transform vertex to bone space when no animation is applied
    pub inverse_bind_matrix: TransformMatrix,
    pub local_bind_transform: TransformBundle,
    pub children: SmallVec<[usize; 4]>,
}

pub type JointIndex = usize;

pub struct Skin {
    // Map from node index to index
    joint_map: BTreeMap<JointIndex, usize>,
    root: JointIndex,
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
                        inverse_bind_matrix: TransformMatrix(Mat4::new(
                            ibm[0].into(),
                            ibm[1].into(),
                            ibm[2].into(),
                            ibm[3].into(),
                        )),
                        local_bind_transform: TransformBundle {
                            pos: Position(Vec3::from(transform.0)),
                            rot: Rotation(Rotor3::from_quaternion_array(transform.1)),
                            scale: Scale(Vec3::from(transform.2)),
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

        let root = armature
            .children()
            .find(|val| joint_map.contains_key(&val.index()))
            .ok_or(Error::MissingRoot)?
            .index();

        Ok(Self {
            joint_map,
            joints,
            root,
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
    pub fn root(&self) -> usize {
        self.root
    }
}
