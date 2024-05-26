use flax::{component, Debuggable, Entity};
use glam::Mat4;
use ivy_assets::Asset;

use crate::{
    driver::WindowHandle, graphics::texture::Texture, material::MaterialDesc, mesh::MeshDesc,
    shader::ShaderDesc,
};

component! {
    /// The gpu texture to use for rendering
    pub(crate) texture: Asset<Texture>,

    pub projection_matrix: Mat4 => [ Debuggable ],

    pub mesh: MeshDesc,
    pub material: MaterialDesc,
    pub shader: Asset<ShaderDesc>,

    pub main_window: (),

    pub window: WindowHandle,
}
