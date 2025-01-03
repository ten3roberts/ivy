use flax::{Entity, Query, World};
use glam::{Mat4, Quat, Vec3};
use ivy_assets::{fs::AssetPath, Asset, AssetCache, DynAsyncAssetDesc};
use ivy_core::{
    app::PostInitEvent,
    layer::events::EventRegisterContext,
    palette::Srgb,
    profiling::ProfilingLayer,
    update_layer::{FixedTimeStep, ScheduledLayer},
    App, AsyncCommandBuffer, EngineLayer, EntityBuilderExt, Layer, DEG_90,
};
use ivy_engine::{async_commandbuffer, engine, main_camera, TransformBundle};
use ivy_game::free_camera::{setup_camera, FreeCameraPlugin};
use ivy_gltf::Document;
use ivy_input::layer::InputLayer;
use ivy_physics::PhysicsPlugin;
use ivy_postprocessing::preconfigured::{SurfacePbrPipelineDesc, SurfacePbrRenderer};
use ivy_scene::{GltfNodeExt, NodeMountOptions};
use ivy_wgpu::{
    components::{environment_data, projection_matrix},
    driver::WinitDriver,
    events::ResizedEvent,
    layer::GraphicsLayer,
    renderer::EnvironmentData,
};
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use winit::window::WindowAttributes;

const ENABLE_SKYBOX: bool = true;

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
                // .with_inner_size(LogicalSize::new(1920, 1080))
                .with_title("Ivy Physics"),
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
                    hdri: Some(Box::new(AssetPath::new(
                        "hdris/HDR_artificial_planet_close.hdr",
                    ))),
                    ..Default::default()
                },
            ))
        }))
        .with_layer(InputLayer::new())
        .with_layer(LogicLayer)
        .with_layer(
            ScheduledLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(FreeCameraPlugin)
                .with_plugin(PhysicsPlugin::new()),
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
        world: &mut World,
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

        events.subscribe(|_, ctx, resized: &ResizedEvent| {
            if let Some(main_camera) = Query::new(projection_matrix().as_mut())
                .with(main_camera())
                .borrow(ctx.world)
                .first()
            {
                let aspect =
                    resized.physical_size.width as f32 / resized.physical_size.height as f32;
                tracing::info!(%aspect);
                *main_camera = Mat4::perspective_rh(1.0, aspect, 0.1, 1000.0);
            }

            Ok(())
        });

        setup_camera()
            .set(
                environment_data(),
                EnvironmentData::new(
                    Srgb::new(0.0, 0.0, 0.0),
                    0.001,
                    if ENABLE_SKYBOX { 0.0 } else { 1.0 },
                ),
            )
            .spawn(world);

        Ok(())
    }
}
