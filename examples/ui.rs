use flax::{Query, World};
use glam::{vec3, Mat4, Quat, Vec3};
use ivy_assets::AssetCache;
use ivy_core::{
    layer::events::EventRegisterContext,
    palette::Srgb,
    profiling::ProfilingLayer,
    update_layer::{FixedTimeStep, PerTick, ScheduledLayer},
    App, EngineLayer, EntityBuilderExt, Layer,
};
use ivy_engine::{main_camera, TransformBundle};
use ivy_game::{
    free_camera::{setup_camera, CameraInputPlugin},
    ray_picker::RayPickingPlugin,
};
use ivy_input::layer::InputLayer;
use ivy_physics::PhysicsPlugin;
use ivy_postprocessing::preconfigured::{SurfacePbrPipeline, SurfacePbrPipelineDesc};
use ivy_wgpu::{
    components::*, driver::WinitDriver, events::ResizedEvent, layer::GraphicsLayer,
    renderer::EnvironmentData,
};
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use violet::core::{
    widget::{col, label},
    Widget,
};
use winit::{dpi::LogicalSize, window::WindowAttributes};

const ENABLE_SKYBOX: bool = true;

pub fn main() -> anyhow::Result<()> {
    color_backtrace::install();

    registry()
        .with(EnvFilter::from_default_env())
        .with(
            HierarchicalLayer::default()
                .with_indent_lines(true)
                .with_span_retrace(true),
        )
        .init();

    if let Err(err) = App::builder()
        .with_driver(WinitDriver::new(
            WindowAttributes::default()
                .with_inner_size(LogicalSize::new(1920, 1080))
                .with_title("Ivy Physics"),
        ))
        .with_layer(EngineLayer::new())
        .with_layer(ProfilingLayer::new())
        .with_layer(GraphicsLayer::new(|world, assets, gpu, surface| {
            Ok(SurfacePbrPipeline::new(
                world,
                assets,
                gpu,
                surface,
                SurfacePbrPipelineDesc {
                    hdri: Some(Box::new(
                        "hdris/kloofendal_48d_partly_cloudy_puresky_2k.hdr",
                    )),
                },
            ))
        }))
        .with_layer(InputLayer::new())
        .with_layer(ivy_engine::ivy_violet::UILayer::new(ui_app()))
        .with_layer(LogicLayer)
        .with_layer(ScheduledLayer::new(PerTick).with_plugin(CameraInputPlugin))
        .with_layer(
            ScheduledLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(PhysicsPlugin::new())
                .with_plugin(RayPickingPlugin),
        )
        .run()
    {
        tracing::error!("{err:?}");
        Err(err)
    } else {
        Ok(())
    }
}

pub fn ui_app() -> impl Widget {
    col(label("Hello, Violet"))
}

struct LogicLayer;

impl Layer for LogicLayer {
    fn register(
        &mut self,
        world: &mut World,
        _: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()> {
        events.subscribe(|_, world, _, resized: &ResizedEvent| {
            if let Some(main_camera) = Query::new(projection_matrix().as_mut())
                .with(main_camera())
                .borrow(world)
                .first()
            {
                let aspect =
                    resized.physical_size.width as f32 / resized.physical_size.height as f32;
                *main_camera = Mat4::perspective_rh(1.0, aspect, 0.01, 1000.0);
            }

            Ok(())
        });

        setup_camera()
            .mount(TransformBundle::new(
                vec3(0.0, 20.0, 20.0),
                Quat::IDENTITY,
                Vec3::ONE,
            ))
            .set(
                environment_data(),
                EnvironmentData::new(
                    Srgb::new(0.2, 0.2, 0.3),
                    0.001,
                    if ENABLE_SKYBOX { 0.0 } else { 1.0 },
                ),
            )
            .spawn(world);

        Ok(())
    }
}
