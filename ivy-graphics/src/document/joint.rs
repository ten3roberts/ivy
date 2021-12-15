use std::collections::{btree_map::Iter, BTreeMap};

use gltf::buffer::Data;
use ivy_base::TransformMatrix;
use smallvec::SmallVec;
use ultraviolet::Mat4;

use crate::Result;

pub struct Joint {
    /// Transform vertex to bone space when no animation is applied
    pub inverse_bind_matrix: TransformMatrix,
    pub children: SmallVec<[usize; 4]>,
}

pub struct Skin {
    joints: BTreeMap<usize, Joint>,
}

impl Skin {
    pub fn from_gltf(skin: gltf::Skin, buffers: &[Data]) -> Result<Self> {
        let reader = skin.reader(|buffer| Some(&buffers[buffer.index()]));
        // let joints = SlotMap::with_key();
        // let accessor = skin.inverse_bind_matrices().unwrap();
        // let view = accessor.view().ok_or(Error::SparseAccessor)?;
        // view.reader

        let joints = skin
            .joints()
            .zip(reader.read_inverse_bind_matrices().unwrap())
            .map(|(joint, ibm)| {
                let index = joint.index();

                (
                    index,
                    Joint {
                        inverse_bind_matrix: TransformMatrix(Mat4::new(
                            ibm[0].into(),
                            ibm[1].into(),
                            ibm[2].into(),
                            ibm[3].into(),
                        )),
                        children: joint.children().map(|val| val.index()).collect(),
                    },
                )
            });

        let joints = joints.collect();

        Ok(Self { joints })
    }

    pub fn joint(&self, index: usize) -> Option<&Joint> {
        self.joints.get(&index)
    }

    pub fn joints(&self) -> Iter<usize, Joint> {
        self.joints.iter()
    }
}
