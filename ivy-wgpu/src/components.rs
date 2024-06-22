use flax::{component, Debuggable};
use glam::Mat4;
use ivy_assets::Asset;

use crate::{
    driver::WindowHandle, light::PointLight, material_desc::MaterialDesc, mesh_desc::MeshDesc,
    shader::ShaderDesc,
};

component! {
    pub projection_matrix: Mat4 => [ Debuggable ],

    pub mesh: MeshDesc,
    pub material: MaterialDesc,
    pub shader: Asset<ShaderDesc>,

    pub mesh_primitive(entity): (),

    pub main_window: (),

    pub window: WindowHandle,

    pub light: PointLight,
}
