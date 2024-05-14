use flax::{component, Debuggable};
use glam::Mat4;
use ivy_assets::Asset;

use crate::{
    graphics::texture::Texture, material::MaterialDesc, mesh::MeshDesc, shader::ShaderDesc,
};

component! {
    /// The gpu texture to use for rendering
    pub(crate) texture: Asset<Texture>,

    pub projection_matrix: Mat4 => [ Debuggable ],

    pub mesh: Asset<MeshDesc>,
    pub material: Asset<MaterialDesc>,
    pub shader: Asset<ShaderDesc>,
}
