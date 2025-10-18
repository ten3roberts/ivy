use flax::{system, Component, ComponentMut, FetchExt, Query, QueryBorrow};
use glam::{Mat4, Vec2};
use ivy_core::{
    components::engine,
    plugin::{Plugin, PluginContext},
};
use ivy_graphics::camera::{camera_settings, projection_matrix, CameraSettings};
use ivy_wgpu::components::viewport_size;

/// Automatically configure a camera based on the window viewport
pub struct CameraViewportPlugin;

impl Plugin for CameraViewportPlugin {
    fn install(&self, ctx: &mut PluginContext) -> anyhow::Result<()> {
        ctx.schedules
            .per_tick_mut()
            .with_system(resize_cameras_system())
            .with_system(update_camera_projection_system());

        Ok(())
    }
}

#[system(args(window_size=viewport_size().copied().modified()), with_query(Query::new((projection_matrix().as_mut(), camera_settings()))))]
fn resize_cameras_system(
    window_size: Vec2,
    cameras: &mut QueryBorrow<(ComponentMut<Mat4>, Component<CameraSettings>)>,
) {
    let new_aspect = window_size.x / window_size.y;
    for (camera, settings) in cameras {
        *camera = settings.projection().create_projection_matrix(new_aspect);
    }
}

#[system(args(camera_settings=camera_settings().modified(), viewport_size=viewport_size().source(engine())))]
fn update_camera_projection_system(
    camera_settings: &CameraSettings,
    projection_matrix: &mut Mat4,
    viewport_size: &Vec2,
) {
    *projection_matrix = camera_settings
        .projection()
        .create_projection_matrix(viewport_size.x / viewport_size.y);
}
