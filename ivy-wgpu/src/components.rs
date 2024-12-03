use flax::{component, Debuggable};
use glam::Mat4;
use ivy_assets::Asset;
use winit::dpi::{LogicalPosition, LogicalSize};

use crate::{
    driver::WindowHandle,
    light::{LightKind, LightParams},
    material_desc::MaterialDesc,
    mesh_desc::MeshDesc,
    renderer::{shadowmapping::LightShadowData, EnvironmentData},
    shader::ShaderPass,
};

component! {
    pub projection_matrix: Mat4 => [ Debuggable ],

    pub mesh: MeshDesc,
    pub material: MaterialDesc,
    pub forward_pass: Asset<ShaderPass>,
    pub transparent_pass: Asset<ShaderPass>,

    pub shadow_pass: Asset<ShaderPass>,

    pub main_window: (),

    pub window: WindowHandle,

    pub window_cursor_position: LogicalPosition<f32>,
    pub window_size: LogicalSize<f32>,


    pub light_params: LightParams,
    pub light_kind:LightKind,
    pub cast_shadow: (),

    /// Shadow-specific data added from shadow mapping node
    pub light_shadow_data: LightShadowData,

    pub environment_data: EnvironmentData,
}
