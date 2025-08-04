use flax::{component, Debuggable};
use glam::Mat4;
use winit::dpi::{LogicalPosition, LogicalSize};

use crate::{
    driver::WindowHandle,
    effect_desc::RenderEffect,
    light::{LightKind, LightParams},
    mesh_desc::MeshDesc,
    renderer::{shadowmapping::LightShadowData, EnvironmentData},
};

component! {
    pub projection_matrix: Mat4 => [ Debuggable ],

    pub mesh: MeshDesc,

    pub forward_pass: RenderEffect,
    pub transparent_pass: RenderEffect,
    pub shadow_pass: RenderEffect,

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
