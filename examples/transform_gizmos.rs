use flax::{
    components::{child_of, name},
    fetch::{FromRelation, Source},
    system, Component, Entity, FetchExt, Query, QueryBorrow, World,
};
use glam::{EulerRot, Mat4, Quat, Vec3};
use ivy_assets::{stored::DynamicStore, AssetCache, AssetPath};
use ivy_core::{
    gizmos::{Gizmos, LineGizmo},
    palette::Srgb,
    profiling::ProfilingLayer,
    transforms::TransformUpdatePlugin,
    update_layer::{FixedTimeStep, Plugin, PluginLayer},
    App, Color, ColorExt, EngineLayer, EntityBuilderExt,
};
use ivy_editor::{
    plugin::EditorPlugin,
    tools::{physics_tool::PhysicsToolPlugin, transform_tool::TransformToolPlugin},
    tools_controller::ToolsControllerPlugin,
};
use ivy_engine::{gizmos, world_transform, RigidBodyBundle, TransformBundle};
use ivy_game::{fly_camera::FlyCameraPlugin, viewport_camera::CameraViewportPlugin};
use ivy_graphics::texture::TextureData;
use ivy_input::layer::InputLayer;
use ivy_physics::{ColliderBundle, GizmoSettings, PhysicsPlugin};
use ivy_postprocessing::preconfigured::{
    pbr::{PbrRenderGraphConfig, SkyboxConfig},
    SurfacePbrPipelineDesc, SurfacePbrRenderer,
};
use ivy_scene::ray_picker::RayPickingPlugin;
use ivy_ui::{
    layer::{UiLayer, UiUpdateLayer},
    streamed::StreamedUiPlugin,
};
use ivy_wgpu::{
    components::{cast_shadow, forward_pass, light_kind, light_params},
    driver::WinitDriver,
    effect_desc::{PbrRenderEffect, RenderEffect},
    layer::GraphicsLayer,
    light::{LightKind, LightParams},
    mesh_desc::MeshDesc,
    primitives::{CubePrimitive, UvSpherePrimitive},
    renderer::RenderObjectBundle,
};
use rand::{rngs::StdRng, Rng, SeedableRng};
use rand_distr::UnitSphere;
use rapier3d::prelude::SharedShape;
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use violet::palette::Srgba;
use wgpu::TextureFormat;
use winit::{dpi::LogicalSize, window::WindowAttributes};

const ENABLE_SKYBOX: bool = true;

pub fn main() -> anyhow::Result<()> {
    color_backtrace::install();

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
                        skybox: None,
                        // skybox: Some(SkyboxConfig {
                        //     hdri: Box::new(AssetPath::new(
                        //         "hdris/kloofendal_48d_partly_cloudy_puresky_2k.hdr",
                        //     )),
                        //     format: TextureFormat::Rgba16Float,
                        // }),
                        hdr_format: Some(wgpu::TextureFormat::Rgba16Float),
                    },
                    ..Default::default()
                },
            ))
        }))
        .with_layer(UiLayer::new())
        .with_layer(InputLayer::new())
        .with_layer(
            PluginLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(FlyCameraPlugin)
                .with_plugin(StreamedUiPlugin)
                .with_plugin(ExamplePlugin)
                .with_plugin(TransformToolPlugin)
                .with_plugin(PhysicsToolPlugin)
                .with_plugin(ToolsControllerPlugin)
                .with_plugin(RayPickingPlugin)
                .with_plugin(EditorPlugin)
                .with_plugin(
                    PhysicsPlugin::new()
                        .with_gravity(Vec3::ZERO)
                        .with_gizmos(GizmoSettings { rigidbody: true }),
                )
                .with_plugin(TransformUpdatePlugin),
        )
        .with_layer(UiUpdateLayer::new())
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

    let material = RenderEffect::Pbr(
        PbrRenderEffect::new()
            .with_roughness_factor(0.1)
            .with_metallic_factor(0.0)
            .with_albedo(TextureData::srgba(Srgba::new(1.0, 1.0, 1.0, 1.0))),
    );

    let metal_material = RenderEffect::Pbr(
        PbrRenderEffect::new()
            .with_roughness_factor(0.2)
            .with_metallic_factor(1.0)
            // gold
            .with_albedo(TextureData::srgba(Srgba::new(0.8, 0.4, 0.2, 1.0))),
    );

    let cube_mesh = MeshDesc::Content(assets.load(&CubePrimitive));
    let sphere_mesh = MeshDesc::Content(assets.load(&UvSpherePrimitive::default()));

    let body = |position: Vec3, rotation: Quat| {
        let mut builder = Entity::builder();
        builder
            .mount(
                TransformBundle::default()
                    .with_position(position)
                    .with_rotation(rotation),
            )
            .mount(
                RigidBodyBundle::dynamic()
                    .with_linear_damping(0.0)
                    .with_angular_damping(0.1),
            );

        builder
    };

    let cube = |position: Vec3, rotation: Quat| {
        let mut builder = body(position, rotation);
        builder
            .set(name(), "Cube".into())
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

    let sphere = |position: Vec3, rotation: Quat| {
        let mut builder = Entity::builder();
        builder
            .set(name(), "Sphere".into())
            .mount(
                TransformBundle::default()
                    .with_position(position)
                    .with_rotation(rotation),
            )
            .mount(
                ColliderBundle::new(SharedShape::ball(1.0))
                    .with_friction(0.7)
                    .with_restitution(0.1),
            )
            .mount(RenderObjectBundle::new(
                sphere_mesh.clone(),
                &[(forward_pass(), metal_material.clone())],
            ));
        builder
    };

    let mut rng = StdRng::seed_from_u64(42);
    for i in 0..10 {
        let v = Vec3::from_array(rng.sample(UnitSphere)) * 10.0;
        let id = cube(v, Quat::IDENTITY)
            .set(name(), format!("Cube {i}"))
            .spawn(world);

        if i % 2 == 0 {
            for _ in 0..i {
                let v = Vec3::from_array(rng.sample(UnitSphere)) * 5.0;
                sphere(v, Quat::IDENTITY)
                    .set_default(child_of(id))
                    .set(name(), format!("Child {i}"))
                    .spawn(world);
            }
        }
    }

    Ok(())
}

struct ExamplePlugin;

impl Plugin for ExamplePlugin {
    fn install(
        &self,
        world: &mut World,
        assets: &AssetCache,
        _: &mut DynamicStore,
        _: &mut ivy_core::update_layer::ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        setup_objects(world, assets)?;

        #[system(with_query(Query::new((world_transform(), world_transform().relation(child_of)))))]
        fn hierarchy_relationship_gizmo_system(
            gizmos: &mut Gizmos,
            query: &mut QueryBorrow<(Component<Mat4>, Source<Component<Mat4>, FromRelation>)>,
        ) {
            let mut section = gizmos.begin_section("hierarchy_relationship_gizmo_system");
            for (transform, parent) in query.iter() {
                let start = transform.transform_point3(Vec3::ZERO);

                let end = parent.transform_point3(Vec3::ZERO);
                section.draw(LineGizmo::from_points(start, end, 0.02, Color::blue()))
            }
        }

        Ok(())
    }
}
