use flax::component;
use ivy_assets::Asset;

use crate::animation::{player::Animator, skin::Skin};

component! {
    pub skin: Asset<Skin>,
    pub animator: Animator,
    pub track_bone: String,
}
