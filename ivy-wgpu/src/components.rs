use flax::component;
use glam::Vec2;
use winit::dpi::LogicalSize;

use crate::{
    driver::WindowHandle,
    effect_desc::RenderEffect,
    light::{LightKind, LightParams},
    mesh_desc::MeshDesc,
    renderer::shadowmapping::LightShadowData,
};

component! {

    pub mesh: MeshDesc,

    pub forward_pass: RenderEffect,
    pub transparent_pass: RenderEffect,
    pub shadow_pass: RenderEffect,

    pub main_window: (),

    pub window: WindowHandle,

    pub window_size: LogicalSize<f32>,
    pub viewport_size: Vec2,


    pub light_params: LightParams,
    pub light_kind:LightKind,
    pub cast_shadow: (),

    /// Shadow-specific data added from shadow mapping node
    pub light_shadow_data: LightShadowData,

}
