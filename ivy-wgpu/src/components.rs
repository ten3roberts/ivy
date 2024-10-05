use flax::{component, Debuggable};
use glam::Mat4;
use ivy_assets::Asset;
use ivy_wgpu_types::PhysicalSize;
use winit::dpi::{LogicalPosition, LogicalSize};

use crate::{
    driver::WindowHandle,
    light::{LightData, LightKind},
    material_desc::MaterialDesc,
    mesh_desc::MeshDesc,
    renderer::{shadowmapping::LightShadowData, EnvironmentData},
    shader::ShaderPassDesc,
};

component! {
    pub projection_matrix: Mat4 => [ Debuggable ],

    pub mesh: MeshDesc,
    pub material: MaterialDesc,
    pub forward_pass: Asset<ShaderPassDesc>,
    pub shadow_pass: Asset<ShaderPassDesc>,

    pub mesh_primitive(entity): (),

    pub main_window: (),

    pub window: WindowHandle,

    pub window_cursor_position: LogicalPosition<f32>,
    pub window_size: LogicalSize<f32>,


    pub light_data: LightData,
    pub light_kind:LightKind,
    pub cast_shadow: (),

    /// Shadow-specific data added from shadow mapping node
    pub light_shadow_data: LightShadowData,

    pub environment_data: EnvironmentData,
}
