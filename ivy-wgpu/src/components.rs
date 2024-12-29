use flax::{component, Debuggable};
use glam::Mat4;
use winit::dpi::{LogicalPosition, LogicalSize};

use crate::{
    driver::WindowHandle,
    light::{LightKind, LightParams},
    material_desc::MaterialData,
    mesh_desc::MeshDesc,
    renderer::{shadowmapping::LightShadowData, EnvironmentData},
};

component! {
    pub projection_matrix: Mat4 => [ Debuggable ],

    pub mesh: MeshDesc,

    pub forward_pass: MaterialData,
    pub transparent_pass: MaterialData,
    pub shadow_pass: MaterialData,


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
