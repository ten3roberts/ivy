use ivy_core::{App, EngineLayer};
use ivy_core::profiling::ProfilingLayer;
use ivy_postprocessing::preconfigured::{pbr::PbrRenderGraphConfig, SurfacePbrPipelineDesc, SurfacePbrRenderer};
use ivy_wgpu::{driver::WinitDriver, layer::GraphicsLayer};
use winit::{dpi::LogicalSize, window::WindowAttributes};

pub fn base_app_builder(title: &str) -> ivy_core::AppBuilder {
    App::builder()
        .with_driver(WinitDriver::new(
            WindowAttributes::default()
                .with_inner_size(LogicalSize::new(1920, 1080))
                .with_title(title),
        ))
        .with_layer(EngineLayer::new())
        .with_layer(ProfilingLayer::new())
}

pub fn graphics_layer_with_config<F>(config_fn: F) -> GraphicsLayer
where
    F: Fn() -> PbrRenderGraphConfig + 'static,
{
    GraphicsLayer::new(move |world, assets, store, gpu, surface| {
        let config = config_fn();
        Ok(SurfacePbrRenderer::new(
            world,
            assets,
            store,
            gpu,
            surface,
            SurfacePbrPipelineDesc {
                pbr_config: config,
                ..Default::default()
            },
        ))
    })
}