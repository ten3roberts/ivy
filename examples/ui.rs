use flax::{
    fetch::Copied, BoxedSystem, Component, Entity, FetchExt, Query, QueryBorrow, System, World,
};
use glam::{vec3, EulerRot, Quat, Vec3};
use itertools::Itertools;
use ivy_assets::{fs::AssetPath, stored::DynamicStore, AssetCache};
use ivy_core::{
    app::PostInitEvent,
    layer::events::EventRegisterContext,
    palette::Srgb,
    profiling::ProfilingLayer,
    transforms::TransformUpdatePlugin,
    update_layer::{FixedTimeStep, Plugin, ScheduleSetBuilder, ScheduledLayer},
    App, Color, ColorExt, EngineLayer, EntityBuilderExt, Layer,
};
use ivy_engine::{
    engine, is_static, main_camera, rotation, scale, RigidBodyBundle, TransformBundle,
};
use ivy_game::{
    fly_camera::{camera_speed, FlyCameraPlugin},
    ray_picker::RayPickingPlugin,
    viewport_camera::{CameraSettings, ViewportCameraLayer},
};
use ivy_graphics::texture::TextureData;
use ivy_input::layer::InputLayer;
use ivy_physics::{components::collider_builder, ColliderBundle, PhysicsPlugin};
use ivy_postprocessing::preconfigured::{
    pbr::{PbrRenderGraphConfig, SkyboxConfig},
    SurfacePbrPipelineDesc, SurfacePbrRenderer,
};
use ivy_scene::editor::hierarchy_panel::HierarchyPanel;
use ivy_ui::{
    layer::{UiLayer, UiUpdateLayer},
    screens::{screen_state, Screen},
    streamed::StreamedUiPlugin,
};
use ivy_wgpu::{
    components::*,
    driver::WinitDriver,
    layer::GraphicsLayer,
    light::{LightKind, LightParams},
    material_desc::{MaterialData, PbrMaterialData},
    mesh_desc::MeshDesc,
    primitives::{CapsulePrimitive, CubePrimitive, UvSpherePrimitive},
    renderer::{EnvironmentData, RenderObjectBundle},
};
use rapier3d::prelude::{ColliderBuilder, RigidBodyType, SharedShape};
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use violet::{
    core::{
        components::LayoutAlignment,
        layout::Align,
        state::StateExt,
        style::{element_accent, SizeExt},
        to_owned,
        unit::Unit,
        widget::*,
        Widget,
    },
    futures_signals::signal::Mutable,
    lucide::icons::{LUCIDE_APP_WINDOW, LUCIDE_LEAF},
    palette::Srgba,
};
use wgpu::TextureFormat;
use winit::{dpi::LogicalSize, window::WindowAttributes};

const ENABLE_SKYBOX: bool = true;

#[derive(Default)]
pub struct UiState {
    camera_speed: f32,
    entity_count: usize,
}

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
                .with_title("Ivy UI"),
        ))
        .with_layer(EngineLayer::new())
        .with_layer(ProfilingLayer::new())
        .with_layer(GraphicsLayer::new(
            move |world, assets, store, gpu, surface| {
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
                                hdri: Box::new(AssetPath::new(
                                    "hdris/kloofendal_48d_partly_cloudy_puresky_2k.hdr",
                                )),
                                format: TextureFormat::Rgba16Float,
                            }),
                            ..Default::default()
                        },
                    },
                ))
            },
        ))
        .with_layer(UiLayer::new())
        .with_layer(InputLayer::new())
        .with_layer(LogicLayer)
        .with_layer(
            ScheduledLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(FlyCameraPlugin)
                .with_plugin(StreamedUiPlugin)
                .with_plugin(ExamplePlugin)
                .with_plugin(PhysicsPlugin::new())
                .with_plugin(RayPickingPlugin)
                .with_plugin(TransformUpdatePlugin),
        )
        .with_layer(ViewportCameraLayer::new(CameraSettings {
            environment_data: EnvironmentData::new(
                Srgb::new(0.2, 0.2, 0.3),
                0.001,
                if ENABLE_SKYBOX { 0.0 } else { 1.0 },
            ),
            fov: 1.0,
        }))
        .with_layer(UiUpdateLayer::new())
        .run()
    {
        tracing::error!("{err:?}");
        Err(err)
    } else {
        Ok(())
    }
}

pub struct MainUi {
    state: Mutable<UiState>,
}

impl Screen for MainUi {
    fn create(self, scope: &mut violet::core::Scope<'_>, _: ivy_ui::screens::ScreenLifetimeToken) {
        let input = Mutable::new("This is some text".to_string());

        let test = card(SignalWidget(self.state.signal_ref(move |v| {
            col((
                HierarchyPanel::new(),
                label(format!("camera speed: {:.1}", v.camera_speed)),
            ))
        })));

        let state = Mutable::new(0);
        let radio_buttons = col((0..4)
            .map(|i| {
                to_owned!(state);
                row(Radio::new(
                    label(format!("{i}")),
                    state.map_value(move |v| v == i, move |_| i),
                ))
            })
            .collect_vec());

        maximized((
            card(Collapsible::label("Scene", HierarchyPanel::new()))
                .with_min_size(Unit::px2(200.0, 0.0))
                .with_item_align(LayoutAlignment::new(Align::Start, Align::Start)),
            card(radio_buttons).with_item_align(LayoutAlignment::new(Align::End, Align::Start)),
            card(row((
                label(LUCIDE_LEAF).with_color(element_accent()),
                label("UI Example"),
                label(LUCIDE_APP_WINDOW),
            )))
            .with_item_align(LayoutAlignment::new(Align::Center, Align::Start)),
        ))
        .mount(scope);
    }
}

struct LogicLayer;

impl Layer for LogicLayer {
    fn register(
        &mut self,
        _: &mut World,
        _: &AssetCache,
        _: &mut DynamicStore,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()> {
        events.subscribe(|_, ctx, _: &PostInitEvent| {
            setup_objects(ctx.world, ctx.assets.clone())?;

            Ok(())
        });

        Ok(())
    }
}

fn setup_objects(world: &mut World, assets: AssetCache) -> anyhow::Result<()> {
    let white_material = MaterialData::PbrMaterial(
        PbrMaterialData::new()
            .with_roughness_factor(1.0)
            .with_metallic_factor(0.0)
            .with_albedo(TextureData::srgba(Srgba::new(1.0, 1.0, 1.0, 1.0))),
    );

    let red_material = MaterialData::PbrMaterial(
        PbrMaterialData::new()
            .with_roughness_factor(1.0)
            .with_metallic_factor(0.0)
            .with_albedo(TextureData::srgba(Color::from_hsla(1.0, 0.7, 0.7, 1.0))),
    );

    const RESTITUTION: f32 = 0.1;
    const FRICTION: f32 = 0.8;

    let body = || {
        let mut builder = Entity::builder();
        builder
            .mount(TransformBundle::default())
            .mount(RigidBodyBundle::new(RigidBodyType::Dynamic))
            .mount(
                ColliderBundle::new(SharedShape::cuboid(1.0, 1.0, 1.0))
                    .with_restitution(RESTITUTION)
                    .with_friction(FRICTION),
            )
            .mount(RenderObjectBundle::new(
                MeshDesc::Content(assets.load(&CubePrimitive)),
                &[
                    (forward_pass(), red_material.clone()),
                    (shadow_pass(), MaterialData::ShadowMaterial),
                ],
            ));

        builder
    };

    let cube = |pos: Vec3, size: Vec3| {
        let mut builder = body();
        builder.set(ivy_core::components::position(), pos).set(
            collider_builder(),
            ColliderBuilder::cuboid(size.x, size.y, size.z),
        );
        builder
    };

    let sphere = |pos: Vec3, size: f32| {
        let mut builder = body();
        builder
            .set(ivy_core::components::position(), pos)
            .set(
                mesh(),
                MeshDesc::Content(assets.load(&UvSpherePrimitive::default())),
            )
            .set(collider_builder(), ColliderBuilder::ball(size));
        builder
    };

    let capsule = |pos: Vec3| {
        let mut builder = body();
        builder
            .set(ivy_core::components::position(), pos)
            .set(
                mesh(),
                MeshDesc::Content(assets.load(&CapsulePrimitive::default())),
            )
            .set(collider_builder(), ColliderBuilder::capsule_y(1.0, 1.0));
        builder
    };

    cube(Vec3::ZERO, vec3(100.0, 1.0, 100.0))
        .mount(RigidBodyBundle::new(RigidBodyType::Fixed))
        .set(scale(), vec3(100.0, 1.0, 100.0))
        .set(is_static(), ())
        .set(forward_pass(), white_material)
        .spawn(world);

    let drop_height = 10.0;

    cube(vec3(0.0, drop_height, 0.0), Vec3::ONE)
        .set(rotation(), Quat::from_scaled_axis(vec3(1.0, 1.0, 0.0)))
        .spawn(world);

    cube(vec3(5.0, drop_height, 0.0), Vec3::ONE)
        .set(rotation(), Quat::from_scaled_axis(vec3(1.0, 0.0, 0.0)))
        .spawn(world);

    sphere(vec3(10.0, drop_height, 0.0), 1.0)
        .set(rotation(), Quat::from_scaled_axis(vec3(0.0, 0.0, 0.0)))
        .spawn(world);

    capsule(vec3(-5.0, drop_height, 0.0))
        .set(rotation(), Quat::from_scaled_axis(vec3(0.1, 0.0, 0.0)))
        .spawn(world);

    capsule(vec3(-10.0, drop_height, 0.0))
        .set(rotation(), Quat::from_scaled_axis(vec3(0.0, 0.0, 1.0)))
        .spawn(world);

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

    Ok(())
}

struct ExamplePlugin;

impl Plugin for ExamplePlugin {
    fn install(
        &self,
        world: &mut World,
        _: &AssetCache,
        _: &mut DynamicStore,
        schedules: &mut ScheduleSetBuilder,
    ) -> anyhow::Result<()> {
        let state = Mutable::new(UiState::default());

        world.get(engine(), screen_state())?.open(MainUi {
            state: state.clone(),
        });

        schedules
            .per_tick_mut()
            .with_system(sync_ui_state_system(state));

        Ok(())
    }
}

fn sync_ui_state_system(state: Mutable<UiState>) -> BoxedSystem {
    System::builder()
        .with_query(Query::new(()))
        .with_query(Query::new(camera_speed().copied()).with(main_camera()))
        .build(
            move |mut all_query: QueryBorrow<()>,
                  mut query: QueryBorrow<Copied<Component<f32>>, _>| {
                let entity_count = all_query.count();
                let camera_speed: f32 = query.first().unwrap_or_default();

                let mut state = state.lock_mut();
                state.entity_count = entity_count;
                state.camera_speed = camera_speed;
            },
        )
        .boxed()
}
