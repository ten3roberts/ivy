use flax::component;
use ivy_assets::Asset;

use crate::graphics::{texture::Texture, Surface};

component! {
    /// The gpu texture to use for rendering
    pub(crate) texture: Asset<Texture>,

    pub(crate) surface: Surface,
}
