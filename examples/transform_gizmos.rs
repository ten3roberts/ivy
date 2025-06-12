use std::{f32::consts::PI, future::ready, sync::Arc};

use flax::{
    components::{child_of, name},
    fetch::{FromRelation, Source},
    system, Component, Entity, FetchExt, Query, QueryBorrow, World,
};
use futures::StreamExt;
use glam::{EulerRot, Mat4, Quat, Vec3};
use ivy_assets::{fs::AssetPath, stored::DynamicStore, AssetCache};
use ivy_core::{
    gizmos::{Gizmos, LineGizmo},
    palette::Srgb,
    profiling::ProfilingLayer,
    transforms::TransformUpdatePlugin,
    update_layer::{FixedTimeStep, Plugin, ScheduledLayer},
    App, Color, ColorExt, EngineLayer, EntityBuilderExt,
};
use ivy_editor::tools::transform_tool::{settings, TransformToolBundle, TransformToolPlugin};
use ivy_engine::{engine, gizmos, world_transform, RigidBodyBundle, TransformBundle};
use ivy_game::{
    fly_camera::FlyCameraPlugin,
    viewport_camera::{CameraSettings, ViewportCameraLayer},
};
use ivy_graphics::texture::TextureData;
use ivy_input::layer::InputLayer;
use ivy_physics::{ColliderBundle, GizmoSettings, PhysicsPlugin};
use ivy_postprocessing::preconfigured::{
    pbr::{PbrRenderGraphConfig, SkyboxConfig},
    SurfacePbrPipelineDesc, SurfacePbrRenderer,
};
use ivy_scene::editor::{hierarchy_panel::HierarchyPanel, manipulator::ManipulationSpace};
use ivy_ui::{
    layer::{UiLayer, UiUpdateLayer},
    screens::{screen_state, Screen, ScreenLifetimeToken},
    streamed::{StreamedUiExt, StreamedUiPlugin},
};
use ivy_wgpu::{
    components::{cast_shadow, forward_pass, light_kind, light_params},
    driver::WinitDriver,
    layer::GraphicsLayer,
    light::{LightKind, LightParams},
    material_desc::{MaterialData, PbrMaterialData},
    mesh_desc::MeshDesc,
    primitives::{CubePrimitive, UvSpherePrimitive},
    renderer::{EnvironmentData, RenderObjectBundle},
};
use rand::{rngs::StdRng, Rng, SeedableRng};
use rand_distr::UnitSphere;
use rapier3d::prelude::SharedShape;
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use violet::{
    core::{
        components::LayoutAlignment,
        layout::Align,
        state::StateExt,
        style::{element_accent, spacing_medium, spacing_small, SizeExt},
        text::TextSegment,
        unit::Unit,
        widget::{card, col, label, maximized, row, Radio, Rectangle, StreamWidget, Text},
        Scope, Widget,
    },
    futures_signals::signal::{Mutable, SignalExt},
    lucide::icons::{LUCIDE_GLOBE, LUCIDE_MOVE_3D, LUCIDE_SCAN_EYE},
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
                .with_inner_size(LogicalSize::new(1280, 720))
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
                .with_plugin(FlyCameraPlugin)
                .with_plugin(StreamedUiPlugin)
                .with_plugin(ExamplePlugin)
                .with_plugin(TransformToolPlugin)
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

    let metal_material = MaterialData::PbrMaterial(
        PbrMaterialData::new()
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
        world.get(engine(), screen_state())?.open(MainUi);

        setup_objects(world, assets)?;

        let manipulator_entity = Entity::builder()
            .mount(TransformToolBundle::default())
            .spawn(world);

        world
            .get(engine(), screen_state())?
            .open(TransformToolScreen {
                id: manipulator_entity,
            });

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

struct MainUi;

impl Screen for MainUi {
    fn create(self, scope: &mut Scope<'_>, _: ScreenLifetimeToken) {
        maximized(col((HierarchyPanel::new().with_item_align(
            LayoutAlignment::new(Align::Start, Align::Center),
        ),)))
        .mount(scope);
    }
}

pub struct TransformToolScreen {
    id: Entity,
}

impl Screen for TransformToolScreen {
    fn create(self, scope: &mut Scope<'_>, _: ScreenLifetimeToken) {
        let state = Mutable::new(None);

        scope.stream_component_duplex(settings(), self.id, state.clone());

        let space_state = Arc::new(
            state
                .clone()
                .lower_option()
                .memo(Default::default())
                .map_ref(|v| &v.space, |v| &mut v.space),
        );

        let settings = state
            .signal_ref(|s| s.clone())
            .to_stream()
            .filter_map(|v| ready(v))
            .map(move |v| {
                col((
                    row((
                        Radio::new_enum(
                            label(LUCIDE_GLOBE),
                            space_state.clone(),
                            ManipulationSpace::Global,
                        ),
                        Radio::new_enum(
                            label(LUCIDE_MOVE_3D),
                            space_state.clone(),
                            ManipulationSpace::Local,
                        ),
                        Radio::new_enum(
                            label(LUCIDE_SCAN_EYE),
                            space_state.clone(),
                            ManipulationSpace::View,
                        ),
                    )),
                    label(format!("Snap Mode: {:?}", v.snap_mode)),
                    label(format!("Angle Snap: {:.2}°", v.angle_snap * 180.0 / PI)),
                    Rectangle::new(Srgba::new(0.2, 0.2, 0.2, 1.0))
                        .with_size(Unit::px2(0.0, 2.0))
                        .with_margin(spacing_medium()),
                    Text::formatted([
                        TextSegment::new("G").with_color(element_accent()),
                        TextSegment::new(" Change Space"),
                    ])
                    .with_margin(spacing_small()),
                    Text::formatted([
                        TextSegment::new("L").with_color(element_accent()),
                        TextSegment::new(" Toggle Snap"),
                    ])
                    .with_margin(spacing_small()),
                ))
                .with_stretch(true)
            });

        maximized(
            col(card(col((label("Transform Tool"), StreamWidget(settings)))))
                .with_item_align(LayoutAlignment::new(Align::End, Align::Start)),
        )
        .mount(scope);
    }
}
