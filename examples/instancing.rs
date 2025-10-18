use std::iter::repeat;

use flax::{component, BatchSpawn, FetchExt, Query, System, World};
use glam::{vec3, Mat4, Quat, Vec3};
use itertools::iproduct;
use ivy_assets::{stored::DynamicStore, AssetCache, AssetPath};
use ivy_core::{
    plugin::{Plugin, PluginContext},
    profiling::ProfilingLayer,
    transforms::TransformUpdatePlugin,
    update_layer::{FixedTimeStep, PluginLayer, ScheduleSetBuilder},
    App, Color, ColorExt, EngineLayer,
};
use ivy_engine::{
    color, elapsed_time, engine, parent_transform, position, rotation, scale, world_transform,
};
use ivy_game::fly_camera::FlyCameraPlugin;
use ivy_gltf::animation::plugin::AnimationPlugin;
use ivy_input::layer::InputLayer;
use ivy_physics::{GizmoSettings, PhysicsPlugin};
use ivy_postprocessing::preconfigured::{
    pbr::{PbrRenderGraphConfig, SkyboxConfig},
    SurfacePbrPipelineDesc, SurfacePbrRenderer,
};
use ivy_wgpu::{
    components::{forward_pass, shadow_pass},
    driver::WinitDriver,
    effect_desc::{PbrRenderEffect, RenderEffect},
    layer::GraphicsLayer,
    mesh_desc::MeshDesc,
    primitives::CubePrimitive,
};
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
                            hdri: Box::new(AssetPath::new(
                                "hdris/kloofendal_48d_partly_cloudy_puresky_2k.hdr",
                            )),
                            format: TextureFormat::Rgba16Float,
                        }),
                        hdr_format: Some(wgpu::TextureFormat::Rgba16Float),
                    },
                    ..Default::default()
                },
            ))
        }))
        .with_layer(InputLayer::new())
        .with_layer(
            PluginLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(LogicPlugin)
                .with_plugin(FlyCameraPlugin)
                .with_plugin(AnimationPlugin)
                .with_plugin(DynamicsPlugin)
                .with_plugin(
                    PhysicsPlugin::new()
                        .with_gravity(Vec3::ZERO)
                        .with_gizmos(GizmoSettings { rigidbody: true }),
                )
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

pub struct LogicPlugin;

impl LogicPlugin {
    fn setup_objects(&self, world: &mut World, assets: &AssetCache) -> anyhow::Result<()> {
        let sphere_mesh = MeshDesc::content(assets.load(&CubePrimitive));

        let plastic_material = RenderEffect::Pbr(
            PbrRenderEffect::new()
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
        builder.set(shadow_pass(), repeat(RenderEffect::OpaqueShadow))?;

        tracing::info!("spawning {} objects", builder.len());
        builder.spawn(world);
        tracing::info!("finished");

        Ok(())
    }
}

impl Plugin for LogicPlugin {
    fn install(&self, ctx: &mut PluginContext) -> anyhow::Result<()> {
        self.setup_objects(ctx.world, ctx.assets)
    }
}

pub struct DynamicsPlugin;

component! {
    rotate_target: (),
}

impl Plugin for DynamicsPlugin {
    fn install(&self, ctx: &mut PluginContext) -> anyhow::Result<()> {
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

        ctx.schedules.per_tick_mut().with_system(rotate_system);

        Ok(())
    }
}
