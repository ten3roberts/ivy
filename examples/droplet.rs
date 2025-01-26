use flax::{Entity, World};
use glam::{Quat, Vec3};
use ivy_assets::{fs::AssetPath, Asset, AssetCache, DynAsyncAssetDesc};
use ivy_core::{
    app::PostInitEvent,
    layer::events::EventRegisterContext,
    palette::Srgb,
    profiling::ProfilingLayer,
    update_layer::{FixedTimeStep, ScheduledLayer},
    App, AsyncCommandBuffer, EngineLayer, EntityBuilderExt, Layer, DEG_90,
};
use ivy_engine::{async_commandbuffer, engine, TransformBundle};
use ivy_game::{
    orbit_camera::OrbitCameraPlugin,
    viewport_camera::{CameraSettings, ViewportCameraLayer},
};
use ivy_gltf::Document;
use ivy_input::layer::InputLayer;
use ivy_physics::PhysicsPlugin;
use ivy_postprocessing::preconfigured::{
    pbr::{PbrRenderGraphConfig, SkyboxConfig},
    SurfacePbrPipelineDesc, SurfacePbrRenderer,
};
use ivy_scene::{GltfNodeExt, NodeMountOptions};
use ivy_wgpu::{driver::WinitDriver, layer::GraphicsLayer, renderer::EnvironmentData};
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use wgpu::TextureFormat;
use winit::window::WindowAttributes;

pub fn main() -> anyhow::Result<()> {
    registry()
        .with(EnvFilter::from_default_env())
        .with(
            HierarchicalLayer::default()
                .with_indent_lines(true)
                .with_deferred_spans(true)
                .with_span_retrace(true),
        )
        .init();

    if let Err(err) = App::builder()
        .with_driver(WinitDriver::new(
            WindowAttributes::default()
                .with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))
                .with_title("Droplet"),
        ))
        .with_layer(EngineLayer::new())
        .with_layer(ProfilingLayer::new())
        .with_layer(GraphicsLayer::new(|world, assets, store, gpu, surface| {
            Ok(SurfacePbrRenderer::new(
                world,
                assets,
                store,
                gpu,
                surface,
                SurfacePbrPipelineDesc {
                    pbr_config: PbrRenderGraphConfig {
                        label: "basic".into(),
                        skybox: Some(SkyboxConfig {
                            hdri: Box::new(AssetPath::new("hdris/HDR_artificial_planet_close.hdr")),
                            format: TextureFormat::Rgba16Float,
                        }),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            ))
        }))
        .with_layer(InputLayer::new())
        .with_layer(LogicLayer)
        .with_layer(
            ScheduledLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(OrbitCameraPlugin)
                .with_plugin(PhysicsPlugin::new()),
        )
        .with_layer(ViewportCameraLayer::new(CameraSettings {
            environment_data: EnvironmentData::new(Srgb::new(0.0, 0.0, 0.1), 0.001, 0.0),
            fov: 1.0,
        }))
        .run()
    {
        tracing::error!("{err:?}");
        Err(err)
    } else {
        Ok(())
    }
}

async fn setup_objects(cmd: AsyncCommandBuffer, assets: AssetCache) -> anyhow::Result<()> {
    let document: Asset<Document> = AssetPath::new("models/droplet.glb")
        .load_async(&assets)
        .await?;

    document
        .node(0)
        .unwrap()
        .mount(
            &mut Entity::builder(),
            &NodeMountOptions {
                skip_empty_children: true,
                material_overrides: &Default::default(),
            },
        )
        .mount(
            TransformBundle::default()
                .with_position(-Vec3::Z)
                .with_rotation(Quat::from_axis_angle(Vec3::Y, -DEG_90)),
        )
        .spawn_into(&mut cmd.lock());

    Ok(())
}

struct LogicLayer;

impl Layer for LogicLayer {
    fn register(
        &mut self,
        _: &mut World,
        _: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()> {
        events.subscribe(|_, ctx, _: &PostInitEvent| {
            async_std::task::spawn(setup_objects(
                ctx.world
                    .get(engine(), async_commandbuffer())
                    .unwrap()
                    .clone(),
                ctx.assets.clone(),
            ));

            Ok(())
        });

        Ok(())
    }
}
