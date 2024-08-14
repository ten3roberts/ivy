use std::sync::Arc;

use anyhow::Context;
use flax::{
    component, BoxedSystem, Component, Entity, EntityBuilder, FetchExt, Mutable, Query,
    QueryBorrow, ScheduleBuilder, System, World,
};
use glam::{vec3, EulerRot, Mat4, Quat, Vec2, Vec3};
use ivy_assets::{Asset, AssetCache};
use ivy_core::{
    app::InitEvent,
    async_commandbuffer, delta_time, engine, gizmos,
    layer::events::EventRegisterContext,
    main_camera,
    palette::{Srgb, WithAlpha},
    profiling::ProfilingLayer,
    rotation,
    update_layer::{FixedTimeStep, PerTick, Plugin, ScheduledLayer},
    velocity, world_transform, App, EngineLayer, EntityBuilderExt, Layer, TransformBundle,
};
use ivy_gltf::{animation::player::Animator, components::animator, Document};
use ivy_graphics::texture::TextureDesc;
use ivy_input::{
    components::input_state,
    layer::InputLayer,
    types::{Key, NamedKey},
    Action, BindingExt, CursorMovement, InputState, KeyBinding, MouseButtonBinding,
};
use ivy_physics::PhysicsPlugin;
use ivy_postprocessing::preconfigured::{PbrRenderGraph, PbrRenderGraphConfig, SkyboxConfig};
use ivy_scene::{GltfNodeExt, NodeMountOptions};
use ivy_wgpu::{
    components::{
        cast_shadow, environment_data, light_data, light_kind, main_window, projection_matrix,
        shadow_pass, window,
    },
    driver::{WindowHandle, WinitDriver},
    events::ResizedEvent,
    layer::GraphicsLayer,
    light::{LightData, LightKind},
    material_desc::{MaterialData, MaterialDesc},
    mesh_desc::MeshDesc,
    primitives::{generate_plane, CubePrimitive, UvSpherePrimitive},
    renderer::{EnvironmentData, RenderObjectBundle},
    rendergraph::{self, ExternalResources, RenderGraph},
    shader_library::{ModuleDesc, ShaderLibrary},
    shaders::{PbrShaderDesc, ShadowShaderDesc},
    Gpu,
};
use ivy_wgpu_types::{PhysicalSize, Surface};
use tracing::Instrument;
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
                .with_title("Ivy"),
        ))
        .with_layer(EngineLayer::new())
        .with_layer(ProfilingLayer::new())
        .with_layer(GraphicsLayer::new(|world, assets, gpu, surface| {
            Ok(RenderGraphRenderer::new(world, assets, gpu, surface))
        }))
        .with_layer(InputLayer::new())
        .with_layer(LogicLayer::new())
        .with_layer(
            ScheduledLayer::new(PerTick)
                .with_plugin(CameraInputPlugin)
                .with_plugin(GizmosPlugin),
        )
        .with_layer(ScheduledLayer::new(FixedTimeStep::new(0.02)).with_plugin(PhysicsPlugin::new()))
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
        schedule.with_system(point_light_gizmo_system());

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

    fn setup_assets(&mut self, world: &mut World, assets: &AssetCache) -> anyhow::Result<()> {
        let cmd = world.get(engine(), async_commandbuffer()).unwrap().clone();
        let assets = assets.clone();

        async_std::task::spawn({
            let cmd = cmd.clone();
            let assets = assets.clone();
            async move {
                let shader = assets.load(&PbrShaderDesc);
                let sphere_mesh = MeshDesc::content(assets.load(&UvSpherePrimitive::default()));
                let materials: Asset<Document> = assets.load_async("textures/materials.glb").await;

                {
                    let mut cmd = cmd.lock();

                    for (i, material) in materials.materials().enumerate() {
                        cmd.spawn(
                            Entity::builder()
                                .mount(TransformBundle::new(
                                    vec3(0.0 + i as f32 * 2.0, 1.0, 12.0),
                                    Quat::IDENTITY,
                                    Vec3::ONE * 0.5,
                                ))
                                .mount(RenderObjectBundle {
                                    mesh: sphere_mesh.clone(),
                                    material: material.into(),
                                    shader: shader.clone(),
                                }),
                        );
                    }
                }
            }
        });

        async_std::task::spawn(
            async move {
                let shader = assets.load(&PbrShaderDesc);

                let plane_mesh = MeshDesc::content(assets.insert(generate_plane(8.0, Vec3::Y)));

                let plane_material = MaterialDesc::content(
                    MaterialData::new()
                        .with_metallic_factor(0.0)
                        .with_albedo(TextureDesc::path(
                            "textures/BaseCollection/Sand/Ground054_1K-PNG_Color.png",
                        ))
                        .with_normal(TextureDesc::path(
                            "textures/BaseCollection/Sand/Ground054_1K-PNG_NormalGL.png",
                        ))
                        .with_metallic_roughness(TextureDesc::path(
                            "textures/BaseCollection/Sand/Ground054_1K-PNG_Roughness.png",
                        ))
                        .with_ambient_occlusion(TextureDesc::path(
                            "textures/BaseCollection/Sand/Ground054_1K-PNG_AmbientOcclusion.png",
                        ))
                        .with_displacement(TextureDesc::path(
                            "textures/BaseCollection/Sand/Ground054_1K-PNG_Displacement.png",
                        )),
                );

                cmd.lock().spawn(
                    Entity::builder()
                        .mount(TransformBundle::new(
                            Vec3::ZERO,
                            Quat::IDENTITY,
                            Vec3::ONE * 2.0,
                        ))
                        .mount(RenderObjectBundle {
                            mesh: plane_mesh.clone(),
                            material: plane_material.clone(),
                            shader: shader.clone(),
                        })
                        .set(shadow_pass(), assets.load(&ShadowShaderDesc)),
                );

                let sphere_mesh = MeshDesc::content(assets.load(&UvSpherePrimitive::default()));
                let cube_mesh = MeshDesc::content(assets.load(&CubePrimitive));

                for i in 0..8 {
                    let roughness = i as f32 / (7) as f32;
                    for j in 0..2 {
                        let metallic = j as f32;

                        let plastic_material = MaterialDesc::content(
                            MaterialData::new()
                                .with_metallic_factor(metallic)
                                .with_roughness_factor(roughness),
                        );

                        cmd.lock().spawn(
                            Entity::builder()
                                .mount(TransformBundle::new(
                                    vec3(0.0 + i as f32 * 2.0, 1.0, 5.0 + 4.0 * j as f32),
                                    Quat::IDENTITY,
                                    Vec3::ONE * 0.5,
                                ))
                                .mount(RenderObjectBundle {
                                    mesh: sphere_mesh.clone(),
                                    material: plastic_material.clone(),
                                    shader: shader.clone(),
                                })
                                .set(shadow_pass(), assets.load(&ShadowShaderDesc)),
                        );
                    }
                }

                for i in 0..8 {
                    let roughness = i as f32 / (7) as f32;
                    for j in 0..2 {
                        let metallic = j as f32;

                        let plastic_material = MaterialDesc::content(
                            MaterialData::new()
                                .with_metallic_factor(metallic)
                                .with_roughness_factor(roughness),
                        );

                        cmd.lock().spawn(
                            Entity::builder()
                                .mount(TransformBundle::new(
                                    vec3(0.0 + i as f32 * 2.0, 5.0, 5.0 + 4.0 * j as f32),
                                    Quat::IDENTITY,
                                    Vec3::ONE * 0.5,
                                ))
                                .mount(RenderObjectBundle {
                                    mesh: cube_mesh.clone(),
                                    material: plastic_material.clone(),
                                    shader: shader.clone(),
                                })
                                .set(shadow_pass(), assets.load(&ShadowShaderDesc)),
                        );
                    }
                }

                tracing::info!("loading spine");
                let document: Asset<Document> = assets.load_async("models/spine.glb").await;
                let node = document
                    .find_node("Cube")
                    .context("Missing document node")
                    .unwrap();

                let skin = node.skin().unwrap();
                let animation = skin.animations()[0].clone();

                let root: EntityBuilder = node
                    .mount(
                        &assets,
                        &mut Entity::builder(),
                        NodeMountOptions { cast_shadow: true },
                    )
                    .mount(TransformBundle::new(
                        vec3(0.0, 0.0, 2.0),
                        Quat::IDENTITY,
                        Vec3::ONE,
                    ))
                    .set(animator(), Animator::new(animation))
                    .into();

                cmd.lock().spawn(root);

                let document: Asset<Document> = assets.load_async("models/crate.glb").await;
                let node = document
                    .find_node("Cube")
                    .context("Missing document node")
                    .unwrap();

                let root: EntityBuilder = node
                    .mount(
                        &assets,
                        &mut Entity::builder(),
                        NodeMountOptions { cast_shadow: true },
                    )
                    .mount(TransformBundle::new(
                        vec3(0.0, 1.0, -2.0),
                        Quat::IDENTITY,
                        Vec3::ONE,
                    ))
                    .into();

                cmd.lock().spawn(root);
            }
            .instrument(tracing::info_span!("load_assets")),
        );

        Ok(())
    }

    fn setup_objects(&mut self, world: &mut World, assets: &AssetCache) -> anyhow::Result<()> {
        self.setup_assets(world, assets)?;

        Entity::builder()
            .mount(
                TransformBundle::default()
                    .with_position(Vec3::Y * 5.0)
                    .with_rotation(Quat::from_euler(EulerRot::YXZ, 0.5, 1.0, 0.0)),
            )
            .set(light_data(), LightData::new(Srgb::new(1.0, 1.0, 1.0), 1.0))
            .set(light_kind(), LightKind::Directional)
            .set_default(cast_shadow())
            .spawn(world);

        Entity::builder()
            .mount(
                TransformBundle::default()
                    .with_position(Vec3::Y * 5.0)
                    .with_rotation(Quat::from_euler(EulerRot::YXZ, 2.0, 0.5, 0.0)),
            )
            .set(light_data(), LightData::new(Srgb::new(1.0, 1.0, 1.0), 1.0))
            .set(light_kind(), LightKind::Directional)
            .set_default(cast_shadow())
            .spawn(world);

        Entity::builder()
            .mount(TransformBundle::default().with_position(vec3(0.0, 2.0, 0.0)))
            .set(light_data(), LightData::new(Srgb::new(1.0, 0.0, 0.0), 25.0))
            .set(light_kind(), LightKind::Point)
            .spawn(world);

        Entity::builder()
            .mount(TransformBundle::default().with_position(vec3(2.0, 2.0, 5.0)))
            .set(light_data(), LightData::new(Srgb::new(0.0, 0.0, 1.0), 25.0))
            .set(light_kind(), LightKind::Point)
            .spawn(world);
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
        events.subscribe(|this, world, assets, InitEvent| this.setup_objects(world, assets));

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
            .mount(TransformBundle::new(Vec3::Y, Quat::IDENTITY, Vec3::ONE))
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
                    .with_action(pan_action),
            )
            .set_default(movement())
            .set_default(rotation_input())
            .set_default(euler_rotation())
            .set_default(pan_active())
            .set(camera_speed(), 5.0)
            .spawn(world);

        Ok(())
    }
}

component! {
    pan_active: f32,
    rotation_input: Vec2,
    euler_rotation: Vec3,
    movement: Vec3,
    camera_speed: f32,
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

fn point_light_gizmo_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new(gizmos().source(engine())))
        .with_query(Query::new((world_transform(), light_data(), light_kind())))
        .build(
            |mut gizmos: QueryBorrow<flax::fetch::Source<Component<gizmos::Gizmos>, Entity>>,
             mut query: QueryBorrow<(
                Component<Mat4>,
                Component<LightData>,
                Component<LightKind>,
            )>| {
                let mut gizmos = gizmos
                    .first()
                    .unwrap()
                    .begin_section("point_light_gizmo_system");

                query
                    .iter()
                    .for_each(|(transform, light, kind)| match kind {
                        LightKind::Point => gizmos.draw(gizmos::Sphere::new(
                            transform.transform_point3(Vec3::ZERO),
                            0.1,
                            light.color.with_alpha(1.0),
                        )),
                        LightKind::Directional => {
                            let pos = transform.transform_point3(Vec3::ZERO);
                            let dir = transform.transform_vector3(Vec3::Z);

                            gizmos.draw(gizmos::Sphere::new(pos, 0.1, light.color.with_alpha(1.0)));

                            gizmos.draw(gizmos::Line::new(
                                pos,
                                dir,
                                0.02,
                                1.0,
                                light.color.with_alpha(1.0),
                            ))
                        }
                    });
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
                hdri: Box::new("hdris/kloofendal_48d_partly_cloudy_puresky_2k.hdr"),
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
