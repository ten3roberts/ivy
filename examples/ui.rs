use bytemuck::cast;
use flax::{
    fetch::Copied, BoxedSystem, Component, Entity, FetchExt, Query, QueryBorrow, System, World,
};
use glam::{vec3, EulerRot, Mat4, Quat, Vec2, Vec3};
use itertools::Itertools;
use ivy_assets::AssetCache;
use ivy_core::{
    app::InitEvent,
    layer::events::EventRegisterContext,
    palette::Srgb,
    profiling::ProfilingLayer,
    update_layer::{FixedTimeStep, PerTick, Plugin, ScheduledLayer, TimeStep},
    App, Color, ColorExt, EngineLayer, EntityBuilderExt, Layer,
};
use ivy_engine::{
    is_static, ivy_ui::UILayer, main_camera, rotation, scale, RigidBodyBundle, TransformBundle,
};
use ivy_game::{
    free_camera::{camera_speed, setup_camera, CameraInputPlugin},
    ray_picker::RayPickingPlugin,
};
use ivy_graphics::texture::TextureDesc;
use ivy_input::layer::InputLayer;
use ivy_physics::{
    components::{collider_shape, rigid_body_type},
    ColliderBundle, PhysicsPlugin,
};
use ivy_postprocessing::preconfigured::{SurfacePbrPipeline, SurfacePbrPipelineDesc};
use ivy_wgpu::{
    components::*,
    driver::WinitDriver,
    events::ResizedEvent,
    layer::GraphicsLayer,
    light::{LightData, LightKind},
    material_desc::{MaterialData, MaterialDesc},
    mesh_desc::MeshDesc,
    primitives::{CapsulePrimitive, CubePrimitive, UvSpherePrimitive},
    renderer::{EnvironmentData, RenderObjectBundle},
    shaders::{PbrShaderDesc, ShadowShaderDesc},
};
use rapier3d::prelude::{RigidBodyType, SharedShape};
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use violet::{
    core::{layout::Alignment, state::State, style::SizeExt, to_owned, widget::*, Widget},
    futures_signals::signal::{Mutable, SignalExt},
    palette::Srgba,
};
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

    let ui_state = Mutable::new(UiState::default());
    let ui_layer = UILayer::new(ui_app(ui_state.clone()));
    let ui_instance = ui_layer.instance().clone();

    if let Err(err) = App::builder()
        .with_driver(WinitDriver::new(
            WindowAttributes::default()
                .with_inner_size(LogicalSize::new(800, 600))
                .with_title("Ivy UI"),
        ))
        .with_layer(EngineLayer::new())
        .with_layer(ProfilingLayer::new())
        .with_layer(GraphicsLayer::new(move |world, assets, gpu, surface| {
            Ok(SurfacePbrPipeline::new(
                world,
                assets,
                gpu,
                surface,
                SurfacePbrPipelineDesc {
                    hdri: Some(Box::new(
                        "hdris/kloofendal_48d_partly_cloudy_puresky_2k.hdr",
                    )),
                    ui_instance: Some(ui_instance.clone()),
                },
            ))
        }))
        .with_layer(InputLayer::new())
        .with_layer(ui_layer)
        .with_layer(LogicLayer)
        .with_layer(
            ScheduledLayer::new(PerTick)
                .with_plugin(CameraInputPlugin)
                .with_plugin(UiStatePlugin {
                    state: ui_state.clone(),
                }),
        )
        .with_layer(
            ScheduledLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(PhysicsPlugin::new())
                .with_plugin(RayPickingPlugin),
        )
        .run()
    {
        tracing::error!("{err:?}");
        Err(err)
    } else {
        Ok(())
    }
}

pub fn ui_app(state: Mutable<UiState>) -> impl Widget {
    let test = card(SignalWidget(state.signal_ref(|v| {
        col((
            label(format!("camera speed: {:.1}", v.camera_speed)),
            label(format!("entity count: {}", v.entity_count)),
        ))
    })));

    let state = Mutable::new(0);
    let radio_buttons = col((0..4)
        .map(|i| {
            to_owned!(state);
            row(Radio::new(
                label(format!("{i}")),
                state.map(move |v| v == i, move |_| i),
            ))
        })
        .collect_vec());

    Stack::new((
        Stack::new(card(test))
            .with_maximize(Vec2::ONE)
            .with_horizontal_alignment(Alignment::Start),
        Stack::new(card(radio_buttons))
            .with_maximize(Vec2::ONE)
            .with_horizontal_alignment(Alignment::End)
            .with_vertical_alignment(Alignment::End),
        Stack::new(card(label("Ivy")))
            .with_maximize(Vec2::ONE)
            .with_horizontal_alignment(Alignment::Center),
    ))
    .with_maximize(Vec2::ONE)
}

struct LogicLayer;

impl Layer for LogicLayer {
    fn register(
        &mut self,
        world: &mut World,
        _: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()> {
        events.subscribe(|_, world, assets, _: &InitEvent| {
            setup_objects(world, assets.clone())?;

            Ok(())
        });

        events.subscribe(|_, world, _, resized: &ResizedEvent| {
            if let Some(main_camera) = Query::new(projection_matrix().as_mut())
                .with(main_camera())
                .borrow(world)
                .first()
            {
                let aspect =
                    resized.physical_size.width as f32 / resized.physical_size.height as f32;
                *main_camera = Mat4::perspective_rh(1.0, aspect, 0.01, 1000.0);
            }

            Ok(())
        });

        setup_camera()
            .mount(TransformBundle::new(
                vec3(0.0, 20.0, 20.0),
                Quat::IDENTITY,
                Vec3::ONE,
            ))
            .set(
                environment_data(),
                EnvironmentData::new(
                    Srgb::new(0.2, 0.2, 0.3),
                    0.001,
                    if ENABLE_SKYBOX { 0.0 } else { 1.0 },
                ),
            )
            .spawn(world);

        Ok(())
    }
}

fn setup_objects(world: &mut World, assets: AssetCache) -> anyhow::Result<()> {
    let white_material = MaterialDesc::Content(
        MaterialData::new()
            .with_roughness_factor(1.0)
            .with_metallic_factor(0.0)
            .with_albedo(TextureDesc::srgba(Srgba::new(1.0, 1.0, 1.0, 1.0))),
    );

    let red_material = MaterialDesc::Content(
        MaterialData::new()
            .with_roughness_factor(1.0)
            .with_metallic_factor(0.0)
            .with_albedo(TextureDesc::srgba(Color::from_hsla(1.0, 0.7, 0.7, 1.0))),
    );

    let shader = assets.load(&PbrShaderDesc);
    let shadow = assets.load(&ShadowShaderDesc);

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
                red_material.clone(),
                shader.clone(),
            ))
            .set(shadow_pass(), shadow.clone());

        builder
    };

    let cube = |pos: Vec3, size: Vec3| {
        let mut builder = body();
        builder.set(ivy_core::components::position(), pos).set(
            collider_shape(),
            SharedShape::cuboid(size.x, size.y, size.z),
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
            .set(collider_shape(), SharedShape::ball(size));
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
            .set(collider_shape(), SharedShape::capsule_y(1.0, 1.0));
        builder
    };

    cube(Vec3::ZERO, vec3(100.0, 1.0, 100.0))
        .set(rigid_body_type(), RigidBodyType::Fixed)
        .set(scale(), vec3(100.0, 1.0, 100.0))
        .set(is_static(), ())
        .set(material(), white_material)
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
        .set(light_data(), LightData::new(Srgb::new(1.0, 1.0, 1.0), 1.0))
        .set(light_kind(), LightKind::Directional)
        .set_default(cast_shadow())
        .spawn(world);

    Ok(())
}

struct UiStatePlugin {
    state: Mutable<UiState>,
}

impl<T: TimeStep> Plugin<T> for UiStatePlugin {
    fn install(
        &self,
        _: &mut World,
        _: &AssetCache,
        schedule: &mut flax::ScheduleBuilder,
        _: &T,
    ) -> anyhow::Result<()> {
        schedule.with_system(sync_ui_state_system(self.state.clone()));

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
