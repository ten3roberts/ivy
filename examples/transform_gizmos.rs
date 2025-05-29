
use flax::{
    components::name,
    system, Entity, World,
};
use glam::{vec3, EulerRot, Mat4, Quat, Vec3};
use ivy_assets::{fs::AssetPath, stored::DynamicStore, AssetCache};
use ivy_core::{
    gizmos::{manipulator::TranslateGizmo, Gizmos},
    palette::Srgb,
    profiling::ProfilingLayer,
    transforms::TransformUpdatePlugin,
    update_layer::{FixedTimeStep, Plugin, ScheduledLayer},
    App, EngineLayer, EntityBuilderExt,
};
use ivy_engine::{engine, gizmos, RigidBodyBundle, TransformBundle};
use ivy_game::{
    orbit_camera::OrbitCameraPlugin,
    viewport_camera::{CameraSettings, ViewportCameraLayer},
};
use ivy_graphics::texture::TextureData;
use ivy_input::layer::InputLayer;
use ivy_physics::{ColliderBundle, GizmoSettings, PhysicsPlugin};
use ivy_postprocessing::preconfigured::{
    pbr::{PbrRenderGraphConfig, SkyboxConfig},
    SurfacePbrPipelineDesc, SurfacePbrRenderer,
};
use ivy_scene::editor::hierarchy_panel::HierarchyPanel;
use ivy_ui::{
    layer::{UiLayer, UiUpdateLayer},
    screens::{screen_state, Screen, ScreenLifetimeToken},
    streamed::StreamedUiPlugin,
};
use ivy_wgpu::{
    components::{cast_shadow, forward_pass, light_kind, light_params},
    driver::WinitDriver,
    layer::GraphicsLayer,
    light::{LightKind, LightParams},
    material_desc::{MaterialData, PbrMaterialData},
    mesh_desc::MeshDesc,
    primitives::CubePrimitive,
    renderer::{EnvironmentData, RenderObjectBundle},
};
use rapier3d::prelude::SharedShape;
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use violet::{
    core::{
        components::LayoutAlignment,
        layout::Align,
        style::SizeExt,
        widget::{card, label, maximized},
        Widget,
    },
    palette::Srgba,
};
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
        .with_layer(UiLayer::new())
        .with_layer(
            ScheduledLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(OrbitCameraPlugin)
                .with_plugin(StreamedUiPlugin)
                .with_plugin(ExamplePlugin)
                .with_plugin(
                    PhysicsPlugin::new()
                        .with_gravity(Vec3::ZERO)
                        .with_gizmos(GizmoSettings { rigidbody: true }),
                )
                .with_plugin(TransformUpdatePlugin),
        )
        .with_layer(UiUpdateLayer::new())
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

fn setup_objects(world: &mut World, assets: &AssetCache) -> anyhow::Result<()> {
    Entity::builder()
        .mount(TransformBundle::default().with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            -2.0,
            -1.0,
            0.0,
        )))
        .set(
            light_params(),
            LightParams::new(Srgb::new(1.0, 1.0, 1.0), 1.0),
        )
        .set(light_kind(), LightKind::Directional)
        .set_default(cast_shadow())
        .spawn(world);

    let material = MaterialData::PbrMaterial(
        PbrMaterialData::new()
            .with_roughness_factor(0.1)
            .with_metallic_factor(0.0)
            .with_albedo(TextureData::srgba(Srgba::new(1.0, 1.0, 1.0, 1.0))),
    );

    let cube_mesh = MeshDesc::Content(assets.load(&CubePrimitive));

    let simulate = true;

    let cube = |mut position: Vec3, rotation: Quat| {
        let mut velocity = Vec3::ZERO;
        if simulate {
            position.z = position.z.signum() * 4.0;

            velocity = Vec3::Z * -position.z.signum() * 1.0;
        }

        let mut builder = Entity::builder();
        builder
            .set(name(), "Cube".into())
            .mount(
                TransformBundle::default()
                    .with_position(position + Vec3::Z)
                    .with_rotation(rotation),
            )
            .mount(RigidBodyBundle::dynamic().with_velocity(velocity))
            .mount(
                ColliderBundle::new(SharedShape::cuboid(1.0, 1.0, 1.0))
                    .with_friction(0.7)
                    .with_restitution(0.1),
            )
            .mount(RenderObjectBundle::new(
                cube_mesh.clone(),
                &[(forward_pass(), material.clone())],
            ));

        builder
    };

    cube(vec3(0.2, 0.0, 0.99), Quat::IDENTITY).spawn(world);

    cube(
        vec3(2.0, 0.0, -0.99),
        Quat::from_scaled_axis(vec3(0.0, 0.0, 0.5)),
    )
    .spawn(world);

    Ok(())
}

struct ExamplePlugin;

impl Plugin for ExamplePlugin {
    fn install(
        &self,
        world: &mut World,
        assets: &AssetCache,
        _: &mut DynamicStore,
        schedules: &mut ivy_core::update_layer::ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        world.get(engine(), screen_state())?.open(MainUi);

        setup_objects(world, assets)?;
        schedules.per_tick_mut().with_system(draw_gizmos_system());

        Ok(())
    }
}

#[system]
fn draw_gizmos_system(gizmos: &mut Gizmos) {
    let mut section = gizmos.begin_section("draw_gizmos_system");

    section.draw(TranslateGizmo {
        transform: Mat4::default(),
    });
}

struct MainUi;

impl Screen for MainUi {
    fn create(self, scope: &mut violet::core::Scope<'_>, _: ScreenLifetimeToken) {
        maximized(card((
            label("Transforms").with_item_align(LayoutAlignment::new(Align::Center, Align::Start)),
            HierarchyPanel::new()
                .with_item_align(LayoutAlignment::new(Align::Start, Align::Center)),
        )))
        .mount(scope);
    }
}
