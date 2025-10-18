use flax::{Entity, World};
use glam::{Quat, Vec3};
use ivy_assets::{stored::DynamicStore, Asset, AssetCache, AssetPath, AsyncAssetExt};
use ivy_core::{
    math::Vec3Ext,
    palette::Srgb,
    profiling::ProfilingLayer,
    transforms::TransformUpdatePlugin,
    update_layer::{FixedTimeStep, Plugin, PluginLayer, ScheduleSetBuilder},
    App, AsyncCommandBuffer, EngineLayer, EntityBuilderExt, DEG_90,
};
use ivy_engine::{async_commandbuffer, engine, TransformBundle};
use ivy_game::orbit_camera::OrbitCameraPlugin;
use ivy_gltf::Document;
use ivy_input::layer::InputLayer;
use ivy_physics::PhysicsPlugin;
use ivy_postprocessing::preconfigured::{
    pbr::{PbrRenderGraphConfig, SkyboxConfig},
    SurfacePbrPipelineDesc, SurfacePbrRenderer,
};
use ivy_scene::{GltfNodeExt, NodeMountOptions};
use ivy_wgpu::{driver::WinitDriver, layer::GraphicsLayer};
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use wgpu::TextureFormat;
use winit::{dpi::LogicalSize, window::WindowAttributes};

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
                .with_inner_size(LogicalSize::new(1920, 1080))
                // .with_fullscreen(Some(winit::window::Fullscreen::Borderless(None)))
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
        .with_layer(
            PluginLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(LogicPlugin)
                .with_plugin(OrbitCameraPlugin)
                .with_plugin(PhysicsPlugin::new())
                .with_plugin(TransformUpdatePlugin),
        )
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
                .with_position(Vec3::FORWARD)
                .with_rotation(Quat::from_axis_angle(Vec3::Y, -DEG_90)),
        )
        .spawn_into(&mut cmd.lock());

    Ok(())
}

struct LogicPlugin;

impl Plugin for LogicPlugin {
    fn install(&self, ctx: &mut PluginContext) -> anyhow::Result<()> {
        let cmd = ctx
            .world
            .get(engine(), async_commandbuffer())
            .unwrap()
            .clone();
        async_std::task::spawn(setup_objects(cmd, ctx.assets.clone()));
        Ok(())
    }
}
