use flax::{component, Debuggable};
use glam::Mat4;
use ivy_assets::Asset;

use crate::{
    driver::WindowHandle, material::MaterialDesc, mesh::MeshDesc, shader::ShaderDesc,
    types::texture::Texture,
};

component! {
    /// The gpu texture to use for rendering
    pub(crate) texture: Asset<Texture>,

    pub projection_matrix: Mat4 => [ Debuggable ],

    pub mesh: MeshDesc,
    pub material: MaterialDesc,
    pub shader: Asset<ShaderDesc>,

    pub mesh_primitive(entity): (),

    pub main_window: (),

    pub window: WindowHandle,
}