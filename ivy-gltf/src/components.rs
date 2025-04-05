use flax::component;
use glam::Mat4;
use ivy_assets::Asset;

use crate::animation::{player::Animator, skin::Skin};

component! {
    pub skin_matrix: Vec<Mat4>,
    pub skin: Asset<Skin>,
    pub animator: Animator,
    pub track_bone: String,
}
