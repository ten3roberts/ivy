use std::iter::repeat;

use flax::{component, BatchSpawn, FetchExt, Query, System, World};
use glam::{vec3, Mat4, Quat, Vec3};
use itertools::iproduct;
use ivy_assets::{fs::AssetPath, AssetCache};
use ivy_core::{
    app::PostInitEvent,
    layer::events::EventRegisterContext,
    palette::Srgb,
    profiling::ProfilingLayer,
    update_layer::{FixedTimeStep, Plugin, ScheduledLayer},
    App, Color, ColorExt, EngineLayer, Layer,
};
use ivy_engine::{
    color, elapsed_time, engine, main_camera, parent_transform, position, rotation, scale,
    world_transform,
};
use ivy_game::free_camera::{setup_camera, FreeCameraPlugin};
use ivy_gltf::animation::plugin::AnimationPlugin;
use ivy_input::layer::InputLayer;
use ivy_physics::{GizmoSettings, PhysicsPlugin};
use ivy_postprocessing::preconfigured::{
    pbr::{PbrRenderGraphConfig, SkyboxConfig},
    SurfacePbrPipelineDesc, SurfacePbrRenderer,
};
use ivy_wgpu::{
    components::{environment_data, forward_pass, projection_matrix, shadow_pass},
    driver::WinitDriver,
    events::ResizedEvent,
    layer::GraphicsLayer,
    material_desc::{MaterialData, PbrMaterialData},
    mesh_desc::MeshDesc,
    primitives::{CubePrimitive, UvSpherePrimitive},
    renderer::EnvironmentData,
};
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use wgpu::TextureFormat;
use winit::{dpi::LogicalSize, window::WindowAttributes};

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
                .with_inner_size(LogicalSize::new(1920, 1080))
                .with_title("Ivy"),
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
                        shadow_map_config: Some(Default::default()),
                        msaa: Some(Default::default()),
                        bloom: Some(Default::default()),
                        skybox: Some(SkyboxConfig {
                            hdri: Box::new(AssetPath::new("hdris/EveningSkyHDRI035B_8K-HDR.exr")),
                            format: TextureFormat::Rgba16Float,
                        }),
                        hdr_format: Some(wgpu::TextureFormat::Rgba16Float),
                    },
                    ..Default::default()
                },
            ))
        }))
        .with_layer(InputLayer::new())
        .with_layer(LogicLayer::new())
        .with_layer(
            ScheduledLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(FreeCameraPlugin)
                .with_plugin(AnimationPlugin)
                .with_plugin(DynamicsPlugin)
                .with_plugin(
                    PhysicsPlugin::new()
                        .with_gravity(Vec3::ZERO)
                        .with_gizmos(GizmoSettings { rigidbody: true }),
                ),
        )
        .run()
    {
        tracing::error!("{err:?}");
        Err(err)
    } else {
        Ok(())
    }
}

pub struct LogicLayer {}

impl Default for LogicLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl LogicLayer {
    pub fn new() -> Self {
        Self {}
    }

    fn setup_objects(&mut self, world: &mut World, assets: &AssetCache) -> anyhow::Result<()> {
        let sphere_mesh = MeshDesc::content(assets.load(&UvSpherePrimitive::default()));

        let plastic_material = MaterialData::PbrMaterial(
            PbrMaterialData::new()
                .with_metallic_factor(1.0)
                .with_roughness_factor(0.4),
        );

        let sidelength = 100;
        let spacing = 25.0;
        let positions = iproduct!(0..sidelength, 0..sidelength, 0..sidelength)
            .map(|(x, y, z)| vec3(x as f32 * spacing, y as f32 * spacing, z as f32 * spacing));

        let transforms = iproduct!(0..sidelength, 0..sidelength, 0..sidelength).map(|(x, y, z)| {
            Mat4::from_translation(vec3(
                x as f32 * spacing,
                y as f32 * spacing,
                z as f32 * spacing,
            ))
        });

        let mut builder = BatchSpawn::new(sidelength * sidelength * sidelength);
        builder.set(position(), positions)?;
        builder.set(rotation(), repeat(Quat::IDENTITY))?;
        builder.set(scale(), repeat(Vec3::ONE))?;
        builder.set(world_transform(), transforms)?;
        builder.set(parent_transform(), repeat(Mat4::IDENTITY))?;
        builder.set(rotate_target(), repeat(()))?;
        builder.set(ivy_wgpu::components::mesh(), repeat(sphere_mesh))?;
        builder.set(color(), repeat(Color::white()))?;
        builder.set(forward_pass(), repeat(plastic_material))?;
        builder.set(shadow_pass(), repeat(MaterialData::ShadowMaterial))?;

        tracing::info!("spawning {} objects", builder.len());
        builder.spawn(world);
        tracing::info!("finished");

        Ok(())
    }
}

impl Layer for LogicLayer {
    fn register(
        &mut self,
        world: &mut World,
        _: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()> {
        events.subscribe(|this, ctx, _: &PostInitEvent| this.setup_objects(ctx.world, ctx.assets));

        events.subscribe(|_, ctx, resized: &ResizedEvent| {
            if let Some(main_camera) = Query::new(projection_matrix().as_mut())
                .with(main_camera())
                .borrow(ctx.world)
                .first()
            {
                let aspect =
                    resized.physical_size.width as f32 / resized.physical_size.height as f32;
                tracing::info!(%aspect);
                *main_camera = Mat4::perspective_rh(1.0, aspect, 0.1, 5000.0);
            }

            Ok(())
        });

        setup_camera()
            .set(
                environment_data(),
                EnvironmentData::new(
                    Srgb::new(0.2, 0.2, 0.3),
                    0.005,
                    if ENABLE_SKYBOX { 0.0 } else { 1.0 },
                ),
            )
            .spawn(world);

        Ok(())
    }
}

pub struct DynamicsPlugin;

component! {
    rotate_target: (),
}

impl Plugin for DynamicsPlugin {
    fn install(
        &self,
        _: &mut World,
        _: &AssetCache,
        schedules: &mut ivy_core::update_layer::ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        let rotate_system = System::builder()
            .with_query(
                Query::new((
                    rotate_target(),
                    rotation().as_mut(),
                    elapsed_time().source(engine()),
                ))
                .batch_size(256),
            )
            .par_for_each(|(_, rotation, elapsed)| {
                *rotation =
                    Quat::from_axis_angle(vec3(1.0, 0.2, 0.0).normalize(), elapsed.as_secs_f32());
            });
        // #[system(args(elapsed=elapsed_time().source(engine())), par)]
        // fn rotate(rotate_target: &(), rotation: &mut Quat, elapsed: &Duration) {}

        // schedules.per_tick_mut().with_system(rotate_system);

        Ok(())
    }
}
