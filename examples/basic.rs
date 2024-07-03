use std::{sync::Arc, time::Instant};

use flax::{
    component, BoxedSystem, Component, Entity, EntityBuilder, Mutable, Query, QueryBorrow,
    Schedule, System, World,
};
use glam::{vec3, EulerRot, Mat4, Quat, Vec2, Vec3};
use image::DynamicImage;
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
    components::input_state, layer::InputLayer, types::Key, Action, Axis3, BindingExt,
    CursorMovement, InputState, KeyBinding, MouseButtonBinding,
};
use ivy_postprocessing::{
    hdri::{HdriProcessor, HdriProcessorNode},
    skybox::SkyboxRenderer,
};
use ivy_scene::GltfNodeExt;
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
    shaders::PbrShaderKey,
    Gpu,
};
use ivy_wgpu_types::{PhysicalSize, Surface};
use tracing::Instrument;
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;
use wgpu::{Extent3d, TextureFormat, TextureUsages};

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

        async_std::task::spawn(
            async move {
                let sphere_mesh = MeshDesc::content(assets.load(&UvSphereDesc::default()));

                let plastic_material = MaterialDesc::content(
                    assets.insert(MaterialData::new().with_metallic(0.0).with_roughness(0.1)),
                );

                let shader = assets.load(&PbrShaderKey);

                cmd.lock().spawn(
                    Entity::builder()
                        .mount(TransformBundle::new(
                            vec3(0.0, 0.0, 5.0),
                            Quat::IDENTITY,
                            Vec3::ONE,
                        ))
                        .mount(RenderObjectBundle {
                            mesh: sphere_mesh,
                            material: plastic_material,
                            shader,
                        }),
                );

                let document: Asset<Document> = assets.load_async("models/Sphere.glb").await;
                tracing::info!("finished loading document");

                let root: EntityBuilder = document
                    .node(0)
                    .unwrap()
                    .mount(&assets, &mut Entity::builder())
                    .mount(TransformBundle::new(
                        vec3(3.0, 0.0, 5.0),
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

        // Entity::builder()
        //     .mount(TransformBundle::default().with_position(vec3(0.0, 100.0, 0.0)))
        //     .set(light(), PointLight::new(Srgb::new(1.0, 1.0, 1.0), 100000.0))
        //     .spawn(world);

        Entity::builder()
            .mount(TransformBundle::default().with_position(vec3(0.0, 50.0, 0.0)))
            .set(light(), PointLight::new(Srgb::new(1.0, 1.0, 1.0), 1000.0))
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
        move_action.add(KeyBinding::new(Key::Character("d".into())).compose(Axis3::X));

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
    final_color: rendergraph::TextureHandle,
    surface_texture: rendergraph::TextureHandle,
}

impl RenderGraphRenderer {
    pub fn new(_world: &mut World, assets: &AssetCache, gpu: &Gpu, surface: Surface) -> Self {
        let size = surface.size();

        let image: Asset<DynamicImage> =
            assets.load("ivy-postprocessing/hdrs/lauter_waterfall_4k.hdr");
        // assets.load("ivy-postprocessing/hdrs/industrial_sunset_puresky_2k.hdr");

        let hdri_processor = HdriProcessor::new(gpu, TextureFormat::Rgba16Float);

        let skybox = Arc::new(hdri_processor.allocate_cubemap(
            gpu,
            PhysicalSize::new(1024, 1024),
            TextureUsages::TEXTURE_BINDING,
        ));

        let skybox_ir = Arc::new(hdri_processor.allocate_cubemap(
            gpu,
            PhysicalSize::new(1024, 1024),
            TextureUsages::TEXTURE_BINDING,
        ));

        let mut render_graph = RenderGraph::new();

        let extent = wgpu::Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        };

        let final_color = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "final_color".into(),
            extent,
            dimension: wgpu::TextureDimension::D2,
            format: surface.surface_config().format,
            mip_level_count: 1,
            sample_count: 4,
        });

        let depth_texture = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "depth_texture".into(),
            extent,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            mip_level_count: 1,
            sample_count: 4,
        });

        let surface_texture = render_graph
            .resources
            .insert_texture(rendergraph::TextureDesc::External);

        let camera_renderer = (SkyboxRenderer::new(gpu), MeshRenderer::new(gpu));

        render_graph.add_node(HdriProcessorNode::new(
            hdri_processor,
            image,
            skybox.clone(),
            skybox_ir.clone(),
        ));

        render_graph.add_node(CameraNode::new(
            gpu,
            depth_texture,
            final_color,
            camera_renderer,
            EnvironmentData::new(skybox, skybox_ir),
        ));
        render_graph.add_node(MsaaResolve::new(final_color, surface_texture));

        Self {
            render_graph,
            surface,
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
        tracing::info!("get next surface texture");
        let surface_texture = self.surface.get_current_texture()?;

        let mut external_resources = ExternalResources::new();
        external_resources.insert_texture(self.surface_texture, &surface_texture.texture);

        self.render_graph
            .draw(gpu, queue, world, assets, &external_resources)?;

        tracing::info!("present");
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
