use flax::{Entity, World};
use glam::{vec3, EulerRot, Quat, Vec3};
use ivy_assets::{fs::AssetPath, AssetCache};
use ivy_core::{
    app::PostInitEvent,
    layer::events::EventRegisterContext,
    palette::{Srgb, Srgba},
    profiling::ProfilingLayer,
    update_layer::{FixedTimeStep, ScheduledLayer},
    App, EngineLayer, EntityBuilderExt, Layer, DEG_180, DEG_45,
};
use ivy_engine::{RigidBodyBundle, TransformBundle};
use ivy_game::{
    fly_camera::FlyCameraPlugin,
    viewport_camera::{CameraSettings, ViewportCameraLayer},
};
use ivy_graphics::texture::TextureData;
use ivy_input::layer::InputLayer;
use ivy_physics::{ColliderBundle, PhysicsPlugin};
use ivy_postprocessing::preconfigured::{
    pbr::{PbrRenderGraphConfig, SkyboxConfig},
    SurfacePbrPipelineDesc, SurfacePbrRenderer,
};
use ivy_wgpu::{
    components::{cast_shadow, forward_pass, light_kind, light_params},
    driver::WinitDriver,
    layer::GraphicsLayer,
    light::{LightKind, LightParams},
    material_desc::{MaterialData, PbrMaterialData},
    mesh_desc::MeshDesc,
    primitives::CapsulePrimitive,
    renderer::{EnvironmentData, RenderObjectBundle},
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
                .with_plugin(FlyCameraPlugin)
                .with_plugin(
                    PhysicsPlugin::new()
                        .with_gravity(Vec3::ZERO)
                        .with_gizmos(ivy_physics::GizmoSettings { rigidbody: true }),
                ),
        )
        .with_layer(ViewportCameraLayer::new(CameraSettings {
            environment_data: EnvironmentData::new(
                Srgb::new(0.2, 0.2, 0.3),
                0.001,
                if ENABLE_SKYBOX { 0.0 } else { 1.0 },
            ),
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

fn setup_objects(world: &mut World, assets: AssetCache) -> anyhow::Result<()> {
    let material = MaterialData::PbrMaterial(
        PbrMaterialData::new()
            .with_roughness_factor(0.1)
            .with_metallic_factor(0.0)
            .with_albedo(TextureData::srgba(Srgba::new(1.0, 1.0, 1.0, 1.0))),
    );

    let cube_mesh = MeshDesc::Content(assets.load(&CapsulePrimitive::default()));

    let simulate = true;

    let mut cube = |mut position: Vec3, rotation: Quat| {
        let mut velocity = Vec3::ZERO;
        if simulate {
            position.z = position.z.signum() * 4.0;

            velocity = Vec3::Z * -position.z.signum() * 1.0;
        }

        Entity::builder()
            .mount(
                TransformBundle::default()
                    .with_position(position + Vec3::Z)
                    .with_rotation(rotation),
            )
            .mount(RigidBodyBundle::dynamic().with_velocity(velocity))
            .mount(
                ColliderBundle::new(rapier3d::prelude::SharedShape::capsule_y(1.0, 1.0))
                    .with_friction(0.7)
                    .with_restitution(0.1),
            )
            .mount(RenderObjectBundle::new(
                cube_mesh.clone(),
                &[(forward_pass(), material.clone())],
            ))
            .spawn(world);
    };

    // Twisted and offset
    cube(vec3(0.2, 0.0, 0.99), Quat::IDENTITY);
    cube(
        vec3(0.0, 0.0, -0.99),
        Quat::from_scaled_axis(vec3(0.0, 0.0, 0.5)),
    );

    // Twisted
    cube(vec3(4.0, 0.0, 0.99), Quat::IDENTITY);
    cube(
        vec3(4.0, 0.0, -0.99),
        Quat::from_scaled_axis(vec3(0.0, 0.0, 0.5)),
    );

    // Offset
    cube(vec3(8.4, 0.0, 0.99), Quat::IDENTITY);
    cube(
        vec3(8.0, 0.0, -0.99),
        Quat::from_scaled_axis(vec3(0.0, 0.0, 0.0)),
    );

    // edge-face
    cube(
        vec3(-4.0, 0.5, 0.9),
        Quat::from_scaled_axis(vec3(0.0, 0.0, DEG_180)),
    );
    cube(
        vec3(-4.0, 0.0, -1.0),
        Quat::from_scaled_axis(vec3(0.0, 0.5, 0.0)),
    );

    // edge-edge
    cube(
        vec3(-8.0, 0.5, 0.8),
        Quat::from_scaled_axis(vec3(DEG_45, 0.0, 0.0)),
    );
    cube(
        vec3(-8.0, 0.0, -0.8),
        Quat::from_scaled_axis(vec3(0.0, 0.5, 0.0)),
    );

    // point-face
    cube(vec3(-12.0, 0.0, 1.3), Quat::IDENTITY);
    cube(
        vec3(-12.0, 0.0, -1.3),
        Quat::from_scaled_axis(vec3(0.5, 0.5, 0.0)),
    );

    Entity::builder()
        .mount(TransformBundle::default().with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            1.0,
            1.0,
            0.0,
        )))
        .set(
            light_params(),
            LightParams::new(Srgb::new(1.0, 1.0, 1.0), 0.4),
        )
        .set(light_kind(), LightKind::Directional)
        .set_default(cast_shadow())
        .spawn(world);

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
            setup_objects(ctx.world, ctx.assets.clone())?;

            Ok(())
        });

        Ok(())
    }
}
