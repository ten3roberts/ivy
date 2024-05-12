use std::sync::Arc;

use flax::{component, Debuggable};
use glam::Mat4;
use ivy_assets::Asset;

use crate::graphics::{material::Material, texture::Texture, Mesh, Shader, Surface};

component! {
    /// The gpu texture to use for rendering
    pub(crate) texture: Asset<Texture>,

    pub projection_matrix: Mat4 => [ Debuggable ],

    pub mesh: Asset<Mesh>,
    pub material: Asset<Material>,
    pub shader: Asset<Shader>,
}
