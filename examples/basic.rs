use std::{f32::consts::TAU, sync::Arc, time::Instant};

use flax::{
    component, BoxedSystem, Component, Entity, EntityBuilder, Mutable, Query, QueryBorrow,
    Schedule, System, World,
};
use glam::{vec3, EulerRot, Mat4, Quat, Vec2, Vec3};
use image::{DynamicImage, Rgba};
use ivy_assets::{Asset, AssetCache};
use ivy_core::{
    app::{InitEvent, TickEvent},
    async_commandbuffer, engine,
    layer::events::EventRegisterContext,
    main_camera,
    palette::Srgb,
    position,
    profiling::ProfilingLayer,
    rotation, App, EngineLayer, EntityBuilderExt, Layer, TransformBundle,
};
use ivy_gltf::Document;
use ivy_input::{
    components::input_state,
    layer::InputLayer,
    types::{Key, NamedKey},
    Action, Axis3, BindingExt, CursorMovement, InputState, KeyBinding, MouseButtonBinding,
};
use ivy_postprocessing::{
    bloom::BloomNode,
    hdri::{HdriProcessor, HdriProcessorNode},
    skybox::SkyboxRenderer,
    tonemap::TonemapNode,
};
use ivy_rendergraph::components::render_graph;
use ivy_scene::GltfNodeExt;
use ivy_vulkan::vk::Extent3D;
use ivy_wgpu::{
    components::{light, main_window, projection_matrix, window},
    driver::{WindowHandle, WinitDriver},
    events::ResizedEvent,
    layer::GraphicsLayer,
    light::PointLight,
    material_desc::{MaterialData, MaterialDesc},
    mesh_desc::MeshDesc,
    primitives::UvSphereDesc,
    renderer::{
        mesh_renderer::MeshRenderer, CameraNode, EnvironmentData, MsaaResolve, RenderObjectBundle,
    },
    rendergraph::{self, ExternalResources, ManagedTextureDesc, RenderGraph},
    shaders::PbrShaderDesc,
    texture::TextureDesc,
    Gpu,
};
use ivy_wgpu_types::{
    texture::{max_mip_levels, read_texture},
    PhysicalSize, Surface,
};
use tracing::Instrument;
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use wgpu::{
    core::binding_model::BindGroupLayoutEntryError, Extent3d, TextureDescriptor, TextureDimension,
    TextureFormat, TextureUsages,
};

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

    let dt = 0.02;

    if let Err(err) = App::builder()
        .with_driver(WinitDriver::new())
        .with_layer(EngineLayer::new())
        .with_layer(ProfilingLayer::new())
        .with_layer(GraphicsLayer::new(|world, assets, gpu, surface| {
            Ok(RenderGraphRenderer::new(world, assets, gpu, surface))
        }))
        .with_layer(InputLayer::new())
        .with_layer(LogicLayer::new())
        .with_layer(Update::new(
            Schedule::builder()
                .with_system(cursor_lock_system())
                .with_system(update_camera_rotation_system())
                .with_system(read_input_rotation_system())
                .build(),
        ))
        .with_layer(FixedUpdate::new(
            dt,
            Schedule::builder().with_system(movement_system(dt)).build(),
        ))
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

    fn setup_assets(&mut self, world: &mut World, assets: &AssetCache) -> anyhow::Result<()> {
        let cmd = world.get(engine(), async_commandbuffer()).unwrap().clone();
        let assets = assets.clone();

        async_std::task::spawn({
            let cmd = cmd.clone();
            let assets = assets.clone();
            async move {
                let shader = assets.load(&PbrShaderDesc);
                let sphere_mesh = MeshDesc::content(assets.load(&UvSphereDesc::default()));
                // let materials:Asset<Document> = assets.load_async("textures/materials.glb").await;

                // {
                //     let mut cmd = cmd.lock();

                //     for (i, material) in materials.materials().enumerate() {

                //         cmd.spawn(
                //             Entity::builder()
                //             .mount(TransformBundle::new(
                //                     vec3(0.0 + i as f32 * 2.0, 5.0, 5.0),
                //                     Quat::IDENTITY,
                //                     Vec3::ONE,
                //             ))
                //             .mount(RenderObjectBundle {
                //                 mesh: sphere_mesh.clone(),
                //                 material: material.into(),
                //                 shader: shader.clone(),
                //             }),
                //         );
                //     }
                // }
            }
        });

        async_std::task::spawn(
            async move {
                let sphere_mesh = MeshDesc::content(assets.load(&UvSphereDesc::default()));
                let shader = assets.load(&PbrShaderDesc);

                for i in 0..8 {
                    let roughness = i as f32 / (7) as f32;
                    for j in 0..2 {
                        let metallic = j as f32;

                        let plastic_material = MaterialDesc::content(
                            MaterialData::new()
                                .with_metallic(metallic)
                                .with_roughness(roughness),
                        );

                        cmd.lock().spawn(
                            Entity::builder()
                                .mount(TransformBundle::new(
                                    vec3(0.0 + i as f32 * 2.0, j as f32 * 2.0, 5.0),
                                    Quat::IDENTITY,
                                    Vec3::ONE,
                                ))
                                .mount(RenderObjectBundle {
                                    mesh: sphere_mesh.clone(),
                                    material: plastic_material.clone(),
                                    shader: shader.clone(),
                                }),
                        );
                    }
                }

                // let document: Asset<Document> = assets.load_async("models/Sphere.glb").await;

                // let root: EntityBuilder = document
                //     .node(0)
                //     .unwrap()
                //     .mount(&assets, &mut Entity::builder())
                //     .mount(TransformBundle::new(
                //         vec3(3.0, 0.0, -2.0),
                //         Quat::IDENTITY,
                //         Vec3::ONE,
                //     ))
                //     .into();

                // cmd.lock().spawn(root);
            }
            .instrument(tracing::info_span!("load_assets")),
        );

        Ok(())
    }

    fn setup_objects(&mut self, world: &mut World, assets: &AssetCache) -> anyhow::Result<()> {
        self.setup_assets(world, assets)?;

        Entity::builder()
            .mount(TransformBundle::default().with_position(vec3(0.0, 2.0, 0.0)))
            .set(light(), PointLight::new(Srgb::new(1.0, 1.0, 1.0), 50.0))
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
                *main_camera = Mat4::perspective_lh(1.0, aspect, 0.1, 1000.0);
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
            .mount(TransformBundle::new(Vec3::ZERO, Quat::IDENTITY, Vec3::ONE))
            .set(main_camera(), ())
            .set_default(projection_matrix())
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

pub struct Update {
    schedule: Schedule,
    current_time: Instant,
}

impl Update {
    pub fn new(schedule: Schedule) -> Self {
        Self {
            schedule,
            current_time: Instant::now(),
        }
    }

    pub fn tick(&mut self, world: &mut World) -> anyhow::Result<()> {
        let now = Instant::now();

        let _elapsed = now.duration_since(self.current_time);
        self.current_time = now;

        self.schedule.execute_par(world)?;

        Ok(())
    }
}

impl Layer for Update {
    fn register(
        &mut self,
        _: &mut World,
        _: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        events.subscribe(|this, world, _, _: &TickEvent| this.tick(world));

        Ok(())
    }
}
pub struct FixedUpdate {
    dt: f32,
    schedule: Schedule,
    acc: f64,
    current_time: Instant,
}

impl FixedUpdate {
    pub fn new(dt: f32, schedule: Schedule) -> Self {
        Self {
            dt,
            schedule,
            acc: 0.0,
            current_time: Instant::now(),
        }
    }

    pub fn tick(&mut self, world: &mut World) -> anyhow::Result<()> {
        let now = Instant::now();

        let elapsed = now.duration_since(self.current_time);
        self.current_time = now;

        self.acc += elapsed.as_secs_f64();

        while self.acc > self.dt as f64 {
            self.schedule.execute_par(world)?;
            self.acc -= self.dt as f64;
        }

        Ok(())
    }
}

impl Layer for FixedUpdate {
    fn register(
        &mut self,
        _: &mut World,
        _: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        events.subscribe(|this, world, _, _: &TickEvent| this.tick(world));

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

fn read_input_rotation_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            euler_rotation().as_mut(),
            rotation_input(),
            pan_active(),
        )))
        .for_each(|(rotation, rotation_input, &pan_active)| {
            *rotation += pan_active * vec3(rotation_input.y, rotation_input.x, 0.0);
        })
        .boxed()
}

fn update_camera_rotation_system() -> BoxedSystem {
    System::builder()
        .with_query(Query::new((rotation().as_mut(), euler_rotation())))
        .for_each(|(rotation, euler_rotation)| {
            *rotation = Quat::from_euler(EulerRot::YXZ, euler_rotation.y, euler_rotation.x, 0.0);
        })
        .boxed()
}

fn movement_system(dt: f32) -> BoxedSystem {
    System::builder()
        .with_query(Query::new((
            movement(),
            rotation(),
            camera_speed(),
            position().as_mut(),
        )))
        .for_each(move |(&movement, rotation, &camera_speed, position)| {
            *position += *rotation * movement * camera_speed * dt;
        })
        .boxed()
}

struct RenderGraphRenderer {
    render_graph: RenderGraph,
    surface: Surface,
    depth_texture: rendergraph::TextureHandle,
    multisampled_hdr: rendergraph::TextureHandle,
    surface_texture: rendergraph::TextureHandle,
    final_color: rendergraph::TextureHandle,
}

impl RenderGraphRenderer {
    pub fn new(_world: &mut World, assets: &AssetCache, gpu: &Gpu, surface: Surface) -> Self {
        let size = surface.size();

        let image: Asset<DynamicImage> =
            assets.load("ivy-postprocessing/hdrs/lauter_waterfall_4k.hdr");
        // assets.load("ivy-postprocessing/hdrs/kloofendal_puresky_2k.hdr");
        // assets.load("ivy-postprocessing/hdrs/industrial_sunset_puresky_2k.hdr");

        const MAX_REFLECTION_LOD: u32 = 8;
        let hdri_processor =
            HdriProcessor::new(gpu, TextureFormat::Rgba16Float, MAX_REFLECTION_LOD);

        let mut render_graph = RenderGraph::new();

        let skybox = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "hdr_cubemap".into(),
            extent: Extent3d {
                width: 1024,
                height: 1024,
                depth_or_array_layers: 6,
            },
            mip_level_count: max_mip_levels(1024, 1024),
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: hdri_processor.format(),
            persistent: true,
        });

        let skybox_ir = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "skybox_ir".into(),
            extent: Extent3d {
                width: 128,
                height: 128,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: hdri_processor.format(),
            persistent: true,
        });

        let skybox_specular = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "hdr_cubemap".into(),
            extent: Extent3d {
                width: 128,
                height: 128,
                depth_or_array_layers: 6,
            },
            mip_level_count: MAX_REFLECTION_LOD,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: hdri_processor.format(),
            persistent: true,
        });

        let integrated_brdf = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "integrated_brdf".into(),
            extent: Extent3d {
                width: 1024,
                height: 1024,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba16Float,
            persistent: true,
        });

        let extent = wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        };

        let multisampled_hdr = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "multisampled_hdr".into(),
            extent,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::Rgba16Float,
            mip_level_count: 1,
            sample_count: 4,
            persistent: false,
        });

        let final_color = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "final_color".into(),
            extent,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::Rgba16Float,
            mip_level_count: 1,
            sample_count: 1,
            persistent: false,
        });

        let bloom_result = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "bloom_result".into(),
            extent,
            dimension: wgpu::TextureDimension::D2,
            format: TextureFormat::Rgba16Float,
            mip_level_count: 1,
            sample_count: 1,
            persistent: false,
        });

        let depth_texture = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "depth_texture".into(),
            extent,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            mip_level_count: 1,
            sample_count: 4,
            persistent: false,
        });

        let surface_texture = render_graph
            .resources
            .insert_texture(rendergraph::TextureDesc::External);

        let camera_renderer = (SkyboxRenderer::new(gpu), MeshRenderer::new(gpu));

        render_graph.add_node(HdriProcessorNode::new(
            hdri_processor,
            image,
            skybox,
            skybox_ir,
            skybox_specular,
            integrated_brdf,
        ));

        render_graph.add_node(CameraNode::new(
            gpu,
            depth_texture,
            multisampled_hdr,
            camera_renderer,
            EnvironmentData::new(skybox, skybox_ir, skybox_specular, integrated_brdf),
        ));

        render_graph.add_node(MsaaResolve::new(multisampled_hdr, final_color));
        render_graph.add_node(BloomNode::new(gpu, final_color, bloom_result, 5, 0.005));
        render_graph.add_node(TonemapNode::new(gpu, final_color, surface_texture));

        Self {
            render_graph,
            surface,
            multisampled_hdr,
            final_color,
            surface_texture,
            depth_texture,
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
        let new_extent = Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        };

        self.surface.resize(gpu, size);

        self.render_graph
            .resources
            .get_texture_mut(self.multisampled_hdr)
            .as_managed_mut()
            .unwrap()
            .extent = new_extent;

        self.render_graph
            .resources
            .get_texture_mut(self.final_color)
            .as_managed_mut()
            .unwrap()
            .extent = new_extent;

        self.render_graph
            .resources
            .get_texture_mut(self.depth_texture)
            .as_managed_mut()
            .unwrap()
            .extent = new_extent;
    }
}
