use std::sync::Arc;

use flax::{
    component, BoxedSystem, Component, Entity, FetchExt, Mutable, Query, QueryBorrow,
    ScheduleBuilder, System, World,
};
use glam::{vec3, EulerRot, Mat4, Quat, Vec2, Vec3};
use ivy_assets::AssetCache;
use ivy_collision::{components::collider, Axis};
use ivy_core::{
    app::InitEvent,
    delta_time, engine, gizmos,
    layer::events::EventRegisterContext,
    main_camera,
    palette::{Srgb, Srgba},
    profiling::ProfilingLayer,
    rotation,
    update_layer::{FixedTimeStep, PerTick, Plugin, ScheduledLayer},
    velocity, App, Color, ColorExt, EngineLayer, EntityBuilderExt, Layer, TransformBundle,
    DEFAULT_RADIUS, DEG_45,
};
use ivy_engine::{Collider, RbBundle};
use ivy_gltf::components::animator;
use ivy_graphics::texture::TextureDesc;
use ivy_input::{
    components::input_state,
    layer::InputLayer,
    types::{Key, NamedKey},
    Action, Axis2, BindingExt, CursorMovement, InputState, KeyBinding, MouseButtonBinding,
    ScrollBinding,
};
use ivy_physics::PhysicsPlugin;
use ivy_postprocessing::preconfigured::{PbrRenderGraph, PbrRenderGraphConfig, SkyboxConfig};
use ivy_wgpu::{
    components::{
        cast_shadow, environment_data, light_data, light_kind, main_window, projection_matrix,
        window,
    },
    driver::{WindowHandle, WinitDriver},
    events::ResizedEvent,
    layer::GraphicsLayer,
    light::{LightData, LightKind},
    material_desc::{MaterialData, MaterialDesc},
    mesh_desc::MeshDesc,
    primitives::CubePrimitive,
    renderer::{EnvironmentData, RenderObjectBundle},
    rendergraph::{self, ExternalResources, RenderGraph},
    shader_library::{ModuleDesc, ShaderLibrary},
    shaders::PbrShaderDesc,
    Gpu,
};
use ivy_wgpu_types::{PhysicalSize, Surface};
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
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
        .with_layer(GraphicsLayer::new(|world, assets, gpu, surface| {
            Ok(RenderGraphRenderer::new(world, assets, gpu, surface))
        }))
        .with_layer(InputLayer::new())
        .with_layer(LogicLayer)
        .with_layer(
            ScheduledLayer::new(PerTick)
                .with_plugin(CameraInputPlugin)
                .with_plugin(GizmosPlugin),
        )
        .with_layer(
            ScheduledLayer::new(FixedTimeStep::new(0.02))
                .with_plugin(PhysicsPlugin::new().with_gizmos(true)),
        )
        .run()
    {
        tracing::error!("{err:?}");
        Err(err)
    } else {
        Ok(())
    }
}

pub struct AnimationPlugin;

impl Plugin<PerTick> for AnimationPlugin {
    fn install(
        &self,
        _: &mut World,
        _: &AssetCache,
        schedule: &mut ScheduleBuilder,
        _: &PerTick,
    ) -> anyhow::Result<()> {
        schedule.with_system(animate_system());
        Ok(())
    }
}

pub struct CameraInputPlugin;

impl Plugin<PerTick> for CameraInputPlugin {
    fn install(
        &self,
        _: &mut World,
        _: &AssetCache,
        schedule: &mut ScheduleBuilder,
        _: &PerTick,
    ) -> anyhow::Result<()> {
        schedule
            .with_system(cursor_lock_system())
            .with_system(camera_speed_input_system())
            .with_system(camera_rotation_input_system())
            .with_system(camera_movement_input_system());

        Ok(())
    }
}

pub struct GizmosPlugin;

impl Plugin<PerTick> for GizmosPlugin {
    fn install(
        &self,
        _: &mut World,
        _: &AssetCache,
        schedule: &mut ScheduleBuilder,
        _: &PerTick,
    ) -> anyhow::Result<()> {
        schedule.with_system(gizmos_system());

        Ok(())
    }
}

fn setup_objects(world: &mut World, assets: AssetCache) -> anyhow::Result<()> {
    let material = MaterialDesc::Content(
        MaterialData::new()
            .with_roughness_factor(0.1)
            .with_metallic_factor(0.0)
            .with_albedo(TextureDesc::srgba(Srgba::new(1.0, 1.0, 1.0, 1.0))),
    );

    let cube_mesh = MeshDesc::Content(assets.load(&CubePrimitive));

    let shader = assets.load(&PbrShaderDesc);

    let distance = 1.1;

    Entity::builder()
        .mount(
            TransformBundle::default().with_position(Vec3::X * (distance)), // .with_rotation(Quat::from_scaled_axis(Vec3::Y * 0.3)),
        )
        .mount(RbBundle::default().with_velocity(-Vec3::Z * 0.1))
        .set(
            collider(),
            Collider::cube_from_center(Vec3::ZERO, Vec3::ONE),
        )
        .mount(RenderObjectBundle::new(
            cube_mesh.clone(),
            material.clone(),
            shader.clone(),
        ))
        .spawn(world);

    Entity::builder()
        .mount(
            TransformBundle::default()
                .with_position(Vec3::X * -distance)
                .with_rotation(Quat::from_scaled_axis(vec3(0.0, 0.4, 0.0))),
        )
        .mount(
            RbBundle::default().with_angular_velocity(Vec3::Y * 0.5), // .with_velocity(Vec3::X), // .with_angular_velocity(Vec3::Y * 0.1),
        )
        .mount(RenderObjectBundle::new(cube_mesh, material, shader))
        .set(
            collider(),
            Collider::cube_from_center(Vec3::ZERO, Vec3::ONE),
        )
        .spawn(world);

    Entity::builder()
        .mount(TransformBundle::default().with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            1.0,
            1.0,
            0.0,
        )))
        .set(light_data(), LightData::new(Srgb::new(1.0, 1.0, 1.0), 1.0))
        .set(light_kind(), LightKind::Directional)
        .set_default(cast_shadow())
        .spawn(world);

    Ok(())
}

fn setup_camera(world: &mut World) {
    let mut speed_action = Action::new(camera_speed_delta());
    speed_action.add(ScrollBinding::new().decompose(Axis2::Y));

    let mut move_action = Action::new(movement());
    move_action.add(KeyBinding::new(Key::Character("w".into())).compose(Vec3::Z));
    move_action.add(KeyBinding::new(Key::Character("a".into())).compose(-Vec3::X));
    move_action.add(KeyBinding::new(Key::Character("s".into())).compose(-Vec3::Z));
    move_action.add(KeyBinding::new(Key::Character("d".into())).compose(Vec3::X));

    move_action.add(KeyBinding::new(Key::Character("c".into())).compose(-Vec3::Y));
    move_action.add(KeyBinding::new(Key::Named(NamedKey::Control)).compose(-Vec3::Y));
    move_action.add(KeyBinding::new(Key::Named(NamedKey::Space)).compose(Vec3::Y));

    let mut rotate_action = Action::new(rotation_input());
    rotate_action.add(CursorMovement::new().amplitude(Vec2::ONE * 0.001));

    let mut pan_action = Action::new(pan_active());
    pan_action
        .add(KeyBinding::new(Key::Character("q".into())))
        .add(MouseButtonBinding::new(
            ivy_input::types::MouseButton::Right,
        ));

    Entity::builder()
        .mount(TransformBundle::new(
            vec3(0.0, 10.0, 10.0),
            Quat::IDENTITY,
            Vec3::ONE,
        ))
        .set(main_camera(), ())
        .set_default(projection_matrix())
        .set_default(velocity())
        .set(
            environment_data(),
            EnvironmentData::new(
                Srgb::new(0.2, 0.2, 0.3),
                0.001,
                if ENABLE_SKYBOX { 0.0 } else { 1.0 },
            ),
        )
        .set(
            input_state(),
            InputState::new()
                .with_action(move_action)
                .with_action(rotate_action)
                .with_action(pan_action)
                .with_action(speed_action),
        )
        .set_default(movement())
        .set_default(rotation_input())
        .set(euler_rotation(), vec3(DEG_45, 0.0, 0.0))
        .set_default(pan_active())
        .set(camera_speed(), 5.0)
        .set_default(camera_speed_delta())
        .spawn(world);
}

struct LogicLayer;

impl Layer for LogicLayer {
    fn register(
        &mut self,
        world: &mut World,
        _: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()> {
        events.subscribe(|_, world, assets, InitEvent| {
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
                tracing::info!(%aspect);
                *main_camera = Mat4::perspective_rh(1.0, aspect, 0.1, 1000.0);
            }

            Ok(())
        });

        setup_camera(world);
        Ok(())
    }
}

component! {
    pan_active: f32,
    rotation_input: Vec2,
    euler_rotation: Vec3,
    movement: Vec3,
    camera_speed: f32,
    camera_speed_delta: f32,
}

fn cursor_lock_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new(pan_active()))
        .with_query(Query::new(window().as_mut()).with(main_window()))
        .build(
            |mut query: QueryBorrow<Component<f32>>,
             mut window: QueryBorrow<Mutable<WindowHandle>, _>| {
                query.iter().for_each(|&pan_active| {
                    if let Some(window) = window.first() {
                        window.set_cursor_lock(pan_active > 0.0);
                    }
                });
            },
        )
        .boxed()
}

fn camera_speed_input_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            camera_speed().as_mut(),
            camera_speed_delta().modified(),
        )))
        .for_each(|(speed, &delta)| {
            let change = 2_f32.powf(-delta * 0.05);
            *speed = (*speed * change).clamp(0.1, 100.0);
            tracing::info!("camera speed: {speed} {delta}");
        })
        .boxed()
}

fn camera_rotation_input_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            rotation().as_mut(),
            euler_rotation().as_mut(),
            rotation_input(),
            pan_active(),
        )))
        .for_each(|(rotation, euler_rotation, rotation_input, &pan_active)| {
            *euler_rotation += pan_active * vec3(rotation_input.y, rotation_input.x, 0.0);
            *rotation = Quat::from_euler(EulerRot::YXZ, -euler_rotation.y, -euler_rotation.x, 0.0);
        })
        .boxed()
}

fn camera_movement_input_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            movement(),
            rotation(),
            camera_speed(),
            velocity().as_mut(),
        )))
        .for_each(move |(&movement, rotation, &camera_speed, velocity)| {
            *velocity = *rotation * (movement * vec3(1.0, 1.0, -1.0) * camera_speed);
        })
        .boxed()
}

fn animate_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            animator().as_mut(),
            delta_time()
                .source(engine())
                .expect("delta_time must be present"),
        )))
        .par_for_each(move |(animator, dt)| {
            animator.step(dt.as_secs_f32());
        })
        .boxed()
}

fn gizmos_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new(gizmos().source(engine())))
        .build(
            |mut gizmos: QueryBorrow<flax::fetch::Source<Component<gizmos::Gizmos>, Entity>>| {
                let mut gizmos = gizmos.first().unwrap().begin_section("gizmos_system");

                gizmos.draw(gizmos::Sphere::new(
                    Vec3::ZERO,
                    DEFAULT_RADIUS,
                    Color::red(),
                ));
            },
        )
        .boxed()
}

struct RenderGraphRenderer {
    render_graph: RenderGraph,
    surface: Surface,
    surface_texture: rendergraph::TextureHandle,
    pbr: PbrRenderGraph,
}

impl RenderGraphRenderer {
    pub fn new(world: &mut World, assets: &AssetCache, gpu: &Gpu, surface: Surface) -> Self {
        let mut render_graph = RenderGraph::new();

        let surface_texture = render_graph
            .resources
            .insert_texture(rendergraph::TextureDesc::External);

        let shader_library = ShaderLibrary::new().with_module(ModuleDesc {
            path: "./assets/shaders/pbr_base.wgsl",
            source: &assets.load::<String>(&"shaders/pbr_base.wgsl".to_string()),
        });

        let shader_library = Arc::new(shader_library);

        let pbr = PbrRenderGraphConfig {
            shadow_map_config: Some(Default::default()),
            msaa: Some(Default::default()),
            bloom: Some(Default::default()),
            skybox: Some(SkyboxConfig {
                hdri: Box::new("hdris/HDR_artificial_planet_close.hdr"),
                format: wgpu::TextureFormat::Rgba16Float,
            }),
            hdr_format: wgpu::TextureFormat::Rgba16Float,
        }
        .configure(
            world,
            gpu,
            assets,
            &mut render_graph,
            shader_library.clone(),
            surface_texture,
        );

        Self {
            render_graph,
            surface,
            surface_texture,
            pbr,
        }
    }
}

impl ivy_wgpu::layer::Renderer for RenderGraphRenderer {
    fn draw(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        gpu: &Gpu,
        queue: &wgpu::Queue,
    ) -> anyhow::Result<()> {
        let surface_texture = self.surface.get_current_texture()?;

        let mut external_resources = ExternalResources::new();
        external_resources.insert_texture(self.surface_texture, &surface_texture.texture);

        self.render_graph
            .draw(gpu, queue, world, assets, &external_resources)?;

        surface_texture.present();

        Ok(())
    }

    fn on_resize(&mut self, gpu: &Gpu, size: PhysicalSize<u32>) {
        self.surface.resize(gpu, size);

        self.pbr.set_size(&mut self.render_graph, size);
    }
}
