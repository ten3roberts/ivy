#![allow(dead_code)]
use std::{
    fmt::Display,
    ops::Deref,
    sync::{mpsc, Arc},
    time::Duration,
};

use anyhow::{anyhow, Context};
use collision::{Collider, Cube, Object, Sphere};
use flume::Receiver;
use glfw::{Action, CursorMode, Glfw, Key, WindowEvent};
use graphics::gizmos::GizmoRenderer;
use hecs::*;
use hecs_hierarchy::Hierarchy;
use ivy::{
    core::*,
    graphics::*,
    input::*,
    rendergraph::*,
    resources::*,
    ui::{constraints::*, *},
    vulkan::*,
    *,
};
use ivy_resources::Resources;
use parking_lot::RwLock;
use physics::{
    components::{AngularMass, AngularVelocity, Mass, Resitution, Velocity},
    PhysicsLayer,
};
use postprocessing::pbr::{create_pbr_pipeline, PBRInfo};
use random::rand::SeedableRng;
use random::{rand::rngs::StdRng, Random};
use slotmap::SecondaryMap;
use std::fmt::Write;
use ultraviolet::{Rotor3, Vec3};
use vulkan::vk::CullModeFlags;

use log::*;

const FRAMES_IN_FLIGHT: usize = 2;

struct OverTime<T> {
    func: Box<dyn Fn(Entity, &mut T, f32, f32) + Send + Sync>,
    elapsed: f32,
}

impl<T> OverTime<T>
where
    T: Component,
{
    fn new(func: Box<dyn Fn(Entity, &mut T, f32, f32) + Send + Sync>) -> Self {
        Self { func, elapsed: 0.0 }
    }

    fn update(world: &mut World, dt: f32) {
        world
            .query::<(&mut Self, &mut T)>()
            .iter()
            .for_each(|(e, (s, val))| {
                s.elapsed += dt;
                (s.func)(e, val, s.elapsed, dt);
            });
    }
}

struct Mover {
    translate: InputVector,
    speed: f32,
}

impl Mover {
    fn new(translate: InputVector, speed: f32) -> Self {
        Self { translate, speed }
    }
}

fn move_system(world: &mut World, input: &Input, dt: f32) {
    world
        .query::<(&Mover, &mut Position, &mut Rotation)>()
        .iter()
        .for_each(|(_, (m, p, r))| {
            *p += Position(r.into_matrix() * m.translate.get(input)) * m.speed * dt;
        })
}

struct Periodic<T> {
    func: Box<dyn Fn(Entity, &mut T, usize) + Send + Sync>,
    clock: Clock,
    period: Duration,
    count: usize,
}

impl<T> Periodic<T>
where
    T: Component,
{
    fn _new(period: Duration, func: Box<dyn Fn(Entity, &mut T, usize) + Send + Sync>) -> Self {
        Self {
            func,
            period,
            clock: Clock::new(),
            count: 0,
        }
    }

    fn update(world: &mut World) {
        world
            .query::<(&mut Self, &mut T)>()
            .iter()
            .for_each(|(e, (s, val))| {
                if s.clock.elapsed() >= s.period {
                    s.clock.reset();
                    (s.func)(e, val, s.count);
                    s.count += 1;
                }
            });
    }
}

fn main() -> anyhow::Result<()> {
    Logger {
        show_location: true,
        max_level: LevelFilter::Debug,
    }
    .install();

    // Go up three levels
    ivy_core::normalize_dir(3)?;

    let glfw = Arc::new(RwLock::new(glfw::init(glfw::FAIL_ON_ERRORS)?));

    let mut app = App::builder()
        .try_push_layer(|_, r, _| {
            WindowLayer::new(
                glfw,
                r,
                WindowInfo {
                    // extent: Some(Extent::new(800, 600)),
                    extent: None,

                    resizable: false,
                    mode: WindowMode::Windowed,
                    ..Default::default()
                },
            )
        })?
        .try_push_layer(|w, r, e| -> anyhow::Result<_> {
            Ok(FixedTimeStep::new(
                20.ms(),
                (
                    PhysicsLayer::<[Object; 32]>::new(w, r, e, Vec3::one() * 100.0)?,
                    LogicLayer::new(w, r, e)?,
                ),
            ))
        })?
        .try_push_layer(|w, r, e| VulkanLayer::new(w, r, e))?
        .try_push_layer(|w, r, e| DebugLayer::new(w, r, e, 100.ms()))?
        .build();

    app.run().context("Failed to run application")
}

fn setup_graphics(
    world: &mut World,
    resources: &Resources,
    camera: Entity,
    canvas: Entity,
) -> anyhow::Result<Assets> {
    let window = resources.get_default::<Window>()?;

    let swapchain_info = ivy_vulkan::SwapchainInfo {
        present_mode: vk::PresentModeKHR::MAILBOX,
        image_count: FRAMES_IN_FLIGHT as u32 + 1,
        ..Default::default()
    };

    let context = resources.get_default::<Arc<VulkanContext>>()?;

    let swapchain = resources.insert_default(Swapchain::new(
        context.clone(),
        window.deref(),
        swapchain_info,
    )?)?;

    let swapchain_extent = resources.get(swapchain)?.extent();

    let final_lit = resources.insert(Texture::new(
        context.clone(),
        &TextureInfo {
            extent: swapchain_extent,
            mip_levels: 1,
            usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED | ImageUsage::TRANSFER_SRC,
            ..Default::default()
        },
    )?)?;

    let mut rendergraph = RenderGraph::new(context.clone(), FRAMES_IN_FLIGHT)?;

    resources.insert(FullscreenRenderer)?;
    resources.insert(GizmoRenderer::new(context.clone())?)?;

    resources.insert(MeshRenderer::new(context.clone(), 16, FRAMES_IN_FLIGHT)?)?;

    let image_renderer =
        resources.insert(ImageRenderer::new(context.clone(), 16, FRAMES_IN_FLIGHT)?)?;

    let text_renderer = resources.insert(TextRenderer::new(
        context.clone(),
        16,
        128,
        FRAMES_IN_FLIGHT,
    )?)?;

    let pbr_nodes = rendergraph.add_nodes(create_pbr_pipeline::<GeometryPass, PostProcessingPass>(
        context.clone(),
        world,
        &resources,
        camera,
        swapchain_extent,
        FRAMES_IN_FLIGHT,
        &[],
        &[AttachmentInfo {
            store_op: StoreOp::STORE,
            load_op: LoadOp::DONT_CARE,
            initial_layout: ImageLayout::UNDEFINED,
            final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            resource: final_lit,
        }],
        &[],
        PBRInfo {
            ambient_radience: Vec3::one() * 0.05,
            max_lights: 10,
        },
    )?);

    let gizmo_node = rendergraph.add_node(CameraNode::<GizmoPass, _, _>::new(
        context.clone(),
        resources,
        camera,
        resources.default::<GizmoRenderer>()?,
        &[AttachmentInfo {
            store_op: StoreOp::STORE,
            load_op: LoadOp::LOAD,
            initial_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            resource: final_lit,
        }],
        &[],
        &[world.get::<DepthAttachment>(camera)?.0],
        None,
        &[],
        &[],
        FRAMES_IN_FLIGHT,
    )?);

    let ui_node = rendergraph.add_node(CameraNode::<UIPass, _, _>::new(
        context.clone(),
        resources,
        canvas,
        (image_renderer, text_renderer),
        &[AttachmentInfo {
            store_op: StoreOp::STORE,
            load_op: LoadOp::LOAD,
            initial_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            resource: final_lit,
        }],
        &[],
        &[],
        None,
        &[],
        &[],
        FRAMES_IN_FLIGHT,
    )?);

    rendergraph.add_node(TextUpdateNode::new(text_renderer));

    rendergraph.add_node(SwapchainNode::new(
        context.clone(),
        swapchain,
        final_lit,
        vec![],
        &resources,
    )?);

    // Build renderpasses
    rendergraph.build(resources.fetch()?, swapchain_extent)?;

    // Create pipelines
    let geometry_node = pbr_nodes[0];
    let post_processing_node = pbr_nodes[1];

    let fullscreen_pipeline = Pipeline::new::<()>(
        context.clone(),
        &PipelineInfo {
            vertexshader: "./res/shaders/fullscreen.vert.spv".into(),
            fragmentshader: "./res/shaders/post_processing.frag.spv".into(),
            samples: SampleCountFlags::TYPE_1,
            extent: swapchain_extent,
            cull_mode: CullModeFlags::NONE,
            ..rendergraph.pipeline_info(post_processing_node)?
        },
    )?;

    let gizmo_pipeline = Pipeline::new::<Vertex>(
        context.clone(),
        &PipelineInfo {
            vertexshader: "./res/shaders/gizmos.vert.spv".into(),
            blending: true,
            fragmentshader: "./res/shaders/gizmos.frag.spv".into(),
            samples: SampleCountFlags::TYPE_1,
            extent: swapchain_extent,
            cull_mode: CullModeFlags::NONE,
            ..rendergraph.pipeline_info(gizmo_node)?
        },
    )?;

    // Create a pipeline from the shaders
    let pipeline = Pipeline::new::<Vertex>(
        context.clone(),
        &PipelineInfo {
            vertexshader: "./res/shaders/default.vert.spv".into(),
            fragmentshader: "./res/shaders/default.frag.spv".into(),
            samples: SampleCountFlags::TYPE_1,
            extent: swapchain_extent,
            ..rendergraph.pipeline_info(geometry_node)?
        },
    )?;

    let geometry_pass = resources.insert(GeometryPass(pipeline))?;

    // Insert one default post processing pass
    resources.insert_default(PostProcessingPass(fullscreen_pipeline))?;
    resources.insert_default(GizmoPass(gizmo_pipeline))?;

    let context = resources.get_default::<Arc<VulkanContext>>()?;

    // Create a pipeline from the shaders
    let ui_pipeline = Pipeline::new::<UIVertex>(
        context.clone(),
        &PipelineInfo {
            blending: true,
            vertexshader: "./res/shaders/ui.vert.spv".into(),
            fragmentshader: "./res/shaders/ui.frag.spv".into(),
            samples: SampleCountFlags::TYPE_1,
            extent: swapchain_extent,
            polygon_mode: vk::PolygonMode::FILL,
            cull_mode: vk::CullModeFlags::NONE,
            front_face: vk::FrontFace::CLOCKWISE,
            ..rendergraph.pipeline_info(ui_node)?
        },
    )?;

    // Create a pipeline from the shaders
    let text_pipeline = Pipeline::new::<UIVertex>(
        context.clone(),
        &PipelineInfo {
            blending: true,
            vertexshader: "./res/shaders/text.vert.spv".into(),
            fragmentshader: "./res/shaders/text.frag.spv".into(),
            samples: SampleCountFlags::TYPE_1,
            extent: swapchain_extent,
            polygon_mode: vk::PolygonMode::FILL,
            cull_mode: vk::CullModeFlags::NONE,
            front_face: vk::FrontFace::CLOCKWISE,
            ..rendergraph.pipeline_info(ui_node)?
        },
    )?;

    let ui_pass = resources.insert(UIPass(ui_pipeline))?;
    let text_pass = resources.insert(UIPass(text_pipeline))?;

    _setup_ui(world, resources, ui_pass, text_pass)?;
    resources.insert_default(rendergraph)?;

    Ok(Assets {
        geometry_pass,
        text_pass,
        ui_pass,
    })
}

fn setup_objects(
    world: &mut World,
    resources: &Resources,
    assets: &Assets,
    camera: Entity,
) -> anyhow::Result<Entities> {
    resources.insert(Gizmos::default())?;

    let document: Handle<Document> = resources
        .load("./res/models/cube.gltf")
        .context("Failed to load cube model")??;

    let cube_mesh = resources.get(document)?.mesh(0);

    let document: Handle<Document> = resources
        .load("./res/models/sphere.gltf")
        .context("Failed to load sphere model")??;

    let sphere_mesh = resources.get(document)?.mesh(0);

    let material: Handle<Material> = resources.load(MaterialInfo {
        albedo: "./res/textures/metal.png".into(),
        roughness: 0.05,
        metallic: 0.9,
        ..Default::default()
    })??;

    world.spawn((
        Position(Vec3::new(0.0, 5.0, 5.0)),
        PointLight::new(1.0, Vec3::new(1.0, 1.0, 0.7) * 5000.0),
    ));

    world.spawn((
        Position(Vec3::new(7.0, 0.0, 0.0)),
        PointLight::new(0.4, Vec3::new(0.0, 0.0, 500.0)),
    ));

    world.spawn((
        Collider::new(Sphere::new(1.0)),
        Color::rgb(1.0, 1.0, 1.0),
        Mass(10.0),
        Velocity::default(),
        Position::new(0.0, 0.5, 0.0),
        Resitution(1.0),
        Scale::uniform(0.5),
        Rotation::euler_angles(0.0, 0.0, 0.0),
        Mover {
            translate: InputVector {
                x: InputAxis::keyboard(Key::L, Key::H),
                y: InputAxis::keyboard(Key::K, Key::J),
                z: InputAxis::keyboard(Key::O, Key::I),
            },
            speed: 3.0,
        },
        sphere_mesh,
        material,
        assets.geometry_pass,
    ));

    world.spawn((
        AngularMass(1.0),
        AngularVelocity::default(),
        Collider::new(Cube::new(1.0)),
        Color::white(),
        Mass(2.0),
        Position::new(-3.0, 0.0, 0.0),
        Resitution(1.0),
        Scale::uniform(0.4),
        Velocity::new(0.5, 0.0, 0.0),
        material,
        assets.geometry_pass,
        cube_mesh,
    ));

    let mut rng = StdRng::seed_from_u64(43);

    const COUNT: usize = 1024;

    world
        .spawn_batch((0..COUNT).map(|_| {
            (
                AngularMass(1.0),
                Collider::new(Sphere::new(1.0)),
                Color::rgb(1.0, 1.0, 1.0),
                Mass(10.0),
                Position(Vec3::rand_uniform(&mut rng) * 10.0),
                Velocity(Vec3::rand_sphere(&mut rng) * 2.0),
                Resitution(1.0),
                Scale::uniform(0.5),
                material,
                assets.geometry_pass,
                sphere_mesh,
            )
        }))
        .for_each(|_| {});

    Ok(Entities { camera })
}

struct Assets {
    geometry_pass: Handle<GeometryPass>,
    text_pass: Handle<UIPass>,
    ui_pass: Handle<UIPass>,
}

struct Entities {
    camera: Entity,
}

struct LogicLayer {
    input: Input,

    camera_euler: Vec3,

    cursor_mode: CursorMode,

    window_events: Receiver<WindowEvent>,
    assets: Assets,
    entities: Entities,
}

impl LogicLayer {
    pub fn new(
        world: &mut World,
        resources: &mut Resources,
        events: &mut Events,
    ) -> anyhow::Result<Self> {
        let window = resources.get_default::<Window>()?;

        let input = Input::new(window.cursor_pos(), events);

        let input_vec = InputVector::new(
            InputAxis::keyboard(Key::D, Key::A),
            InputAxis::keyboard(Key::Space, Key::LeftControl),
            InputAxis::keyboard(Key::S, Key::W),
        );

        let extent = window.extent();

        let camera = world.spawn((
            Camera::perspective(1.0, extent.aspect(), 0.1, 100.0),
            Mover::new(input_vec, 5.0),
            MainCamera,
            Position(Vec3::new(0.0, 0.0, 5.0)),
            Rotation(Rotor3::identity()),
        ));

        let canvas = world.spawn((
            Canvas,
            Size2D(extent.as_vec()),
            Position2D::new(0.0, 0.0),
            Camera::default(),
        ));

        let assets =
            setup_graphics(world, resources, camera, canvas).context("Failed to setup graphics")?;

        let entities = setup_objects(world, resources, &assets, camera)?;

        let (tx, window_events) = flume::unbounded();
        events.subscribe(tx);

        Ok(Self {
            input,
            camera_euler: Vec3::zero(),
            entities,
            assets,
            window_events,
            cursor_mode: CursorMode::Normal,
        })
    }

    pub fn handle_events(
        &mut self,
        world: &mut World,
        resources: &Resources,
    ) -> anyhow::Result<()> {
        let window = resources.get_default_mut::<Window>()?;

        for event in self.window_events.try_iter() {
            match event {
                WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                    if self.cursor_mode == CursorMode::Normal {
                        self.cursor_mode = CursorMode::Disabled;
                    } else {
                        self.cursor_mode = CursorMode::Normal;
                    }

                    window.set_cursor_mode(self.cursor_mode);
                }
                WindowEvent::Focus(false) => {
                    self.cursor_mode = CursorMode::Normal;
                    window.set_cursor_mode(self.cursor_mode)
                }
                WindowEvent::Focus(true) => {
                    self.cursor_mode = CursorMode::Disabled;
                    window.set_cursor_mode(self.cursor_mode)
                }
                WindowEvent::Scroll(_, scroll) => {
                    let mut mover = world.get_mut::<Mover>(self.entities.camera)?;
                    mover.speed = (mover.speed + scroll as f32 * 0.2).clamp(0.1, 20.0);
                }
                _ => {}
            }
        }
        Ok(())
    }
}

impl Layer for LogicLayer {
    fn on_update(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
        _events: &mut Events,
        frame_time: Duration,
    ) -> anyhow::Result<()> {
        let _scope = TimedScope::new(|elapsed| log::trace!("Logic layer took {:.3?}", elapsed));

        self.handle_events(world, resources)
            .context("Failed to handle events")?;

        self.input.on_update();

        let dt = frame_time.secs();

        let (_e, camera_rot) = world
            .query_mut::<&mut Rotation>()
            .with::<Camera>()
            .into_iter()
            .next()
            .unwrap();

        let window = resources.get_default::<Window>()?;

        let mouse_movement = self.input.rel_mouse_pos() / window.extent().as_vec();

        self.camera_euler += mouse_movement.xyz();

        *camera_rot = Rotor3::from_euler_angles(
            self.camera_euler.z,
            self.camera_euler.y,
            -self.camera_euler.x,
        )
        .into();

        // Clear gizmos from last frame
        OverTime::<RelativeOffset>::update(world, dt);
        Periodic::<Text>::update(world);

        move_system(world, &self.input, dt);

        {
            let _scope =
                TimedScope::new(|elapsed| log::trace!("--Graphics updating took {:.3?}", elapsed));
            graphics::systems::satisfy_objects(world);
            graphics::systems::update_view_matrices(world);
        }

        {
            let _scope = TimedScope::new(|elapsed| log::trace!("--UI took {:.3?}", elapsed));

            let canvas = world
                .query::<(&Canvas, &Camera)>()
                .iter()
                .next()
                .ok_or(anyhow!("Missing canvas"))?
                .0;

            ui::systems::statisfy_widgets(world);
            ui::systems::update_canvas(world, canvas)?;
            ui::systems::update(world)?;
        }

        Ok(())
    }
}

new_shaderpass! {
    pub struct GeometryPass;
    pub struct WireframePass;
    pub struct UIPass;
    pub struct GizmoPass;
    pub struct PostProcessingPass;
}

struct DisplayDebugReport;

struct VulkanLayer {
    context: Arc<VulkanContext>,
}

fn _setup_ui(
    world: &mut World,
    resources: &Resources,
    ui_pass: Handle<UIPass>,
    text_pass: Handle<UIPass>,
) -> anyhow::Result<()> {
    let canvas = world
        .query::<&Canvas>()
        .iter()
        .next()
        .ok_or(anyhow!("Missing canvas"))?
        .0;

    let image: Handle<Image> = resources.load(ImageInfo {
        texture: "./res/textures/heart.png".into(),
        sampler: SamplerInfo::default(),
    })??;

    let font: Handle<Font> = resources.load((
        FontInfo {
            size: 64.0,
            ..Default::default()
        },
        "./res/fonts/Lora/Lora-VariableFont_wght.ttf".into(),
    ))??;

    let monospace: Handle<Font> = resources.load((
        FontInfo {
            size: 32.0,
            ..Default::default()
        },
        "./res/fonts/Roboto_Mono/RobotoMono-VariableFont_wght.ttf".into(),
    ))??;

    world.attach_new::<Widget, _>(
        canvas,
        (
            Widget,
            image,
            ui_pass,
            RelativeOffset::new(-0.25, -0.5),
            AbsoluteSize::new(100.0, 100.0),
        ),
    )?;

    let widget2 = world.attach_new::<Widget, _>(
        canvas,
        (
            Widget,
            image,
            ui_pass,
            OverTime::<RelativeOffset>::new(Box::new(|_, offset, elapsed, _| {
                offset.x = (elapsed * 0.25).sin();
            })),
            RelativeOffset::new(0.0, -0.5),
            RelativeSize::new(0.2, 0.2),
        ),
    )?;

    world.attach_new::<Widget, _>(
        widget2,
        (Widget, ui_pass, OffsetSize::new(-10.0, -10.0), image),
    )?;

    world.attach_new::<Widget, _>(
        widget2,
        (
            Widget,
            font,
            Text::new("Hello, World!"),
            TextAlignment {
                horizontal: HorizontalAlign::Center,
                vertical: VerticalAlign::Middle,
            },
            WrapStyle::Word,
            RelativeSize::new(1.0, 1.0),
            text_pass,
            AbsoluteOffset::new(0.0, 0.0),
        ),
    )?;

    world.attach_new::<Widget, _>(
        canvas,
        (
            Widget,
            font,
            Text::new("Ivy"),
            TextAlignment::new(HorizontalAlign::Center, VerticalAlign::Bottom),
            text_pass,
            RelativeOffset::new(0.0, 0.0),
            OffsetSize::new(0.7, 0.7),
        ),
    )?;

    world.attach_new::<Widget, _>(
        canvas,
        (
            Widget,
            monospace,
            text_pass,
            DisplayDebugReport,
            Text::new(""),
            TextAlignment::new(HorizontalAlign::Left, VerticalAlign::Top),
            RelativeOffset::new(0.0, 0.0),
            OffsetSize::new(-10.0, 0.0),
        ),
    )?;

    let satellite = world.attach_new::<Widget, _>(
        widget2,
        (
            Widget,
            image,
            ui_pass,
            OverTime::<RelativeOffset>::new(Box::new(|_, offset, elapsed, _| {
                *offset = RelativeOffset::new((elapsed).cos() * 4.0, elapsed.sin() * 2.0) * 0.5
            })),
            RelativeOffset::default(),
            RelativeSize::new(0.4, 0.4),
        ),
    )?;

    world.attach_new::<Widget, _>(
        satellite,
        (
            Widget,
            image,
            ui_pass,
            OverTime::<RelativeOffset>::new(Box::new(|_, offset, elapsed, _| {
                *offset = RelativeOffset::new(-(elapsed * 5.0).cos(), -(elapsed * 5.0).sin()) * 0.5
            })),
            RelativeOffset::default(),
            AbsoluteSize::new(50.0, 50.0),
        ),
    )?;

    Ok(())
}

impl VulkanLayer {
    pub fn new(_: &mut World, resources: &Resources, _: &mut Events) -> anyhow::Result<Self> {
        let context = resources.get_default::<Arc<VulkanContext>>()?.clone();

        Ok(Self { context })
    }
}

impl Layer for VulkanLayer {
    fn on_update(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
        _events: &mut Events,
        _frame_time: Duration,
    ) -> anyhow::Result<()> {
        TimedScope::new(|elapsed| log::trace!("Vulkan layer took {:.3?}", elapsed));
        let context = resources.get_default::<Arc<VulkanContext>>()?;
        // Ensure gpu side data for cameras
        GpuCameraData::create_gpu_cameras(&context, world, FRAMES_IN_FLIGHT)?;

        let mut rendergraph = resources.get_default_mut::<RenderGraph>()?;

        let current_frame = rendergraph.begin()?;

        resources
            .get_default_mut::<Swapchain>()?
            .acquire_next_image(rendergraph.wait_semaphore(current_frame))?;

        GpuCameraData::update_all_system(world, current_frame)?;
        LightManager::update_all_system(world, current_frame)?;

        rendergraph.execute(world, resources)?;
        rendergraph.end()?;

        // // Present results
        resources.get_default::<Swapchain>()?.present(
            context.present_queue(),
            &[rendergraph.signal_semaphore(current_frame)],
        )?;

        Ok(())
    }
}

impl Drop for VulkanLayer {
    fn drop(&mut self) {
        device::wait_idle(self.context.device()).expect("Failed to wait on device");
    }
}

struct WindowLayer {
    glfw: Arc<RwLock<Glfw>>,
    events: mpsc::Receiver<(f64, WindowEvent)>,
}

impl WindowLayer {
    pub fn new(
        glfw: Arc<RwLock<Glfw>>,
        resources: &Resources,
        info: WindowInfo,
    ) -> anyhow::Result<Self> {
        let (window, events) = Window::new(glfw.clone(), info)?;
        let context = Arc::new(VulkanContext::new(&window)?);

        resources.insert(context)?;
        resources.insert(window)?;

        Ok(Self { glfw, events })
    }
}

impl Layer for WindowLayer {
    fn on_update(
        &mut self,
        _world: &mut World,
        _: &mut Resources,
        events: &mut Events,
        _frame_time: Duration,
    ) -> anyhow::Result<()> {
        let _scope = TimedScope::new(|elapsed| log::trace!("Window layer took {:.3?}", elapsed));
        self.glfw.write().poll_events();

        for (_, event) in glfw::flush_messages(&self.events) {
            if let WindowEvent::Close = event {
                events.send(AppEvent::Exit);
            }

            events.send(event);
        }

        Ok(())
    }
}

#[derive(Debug, Clone)]
struct DebugReport<'a> {
    framerate: f32,
    min_frametime: Duration,
    avg_frametime: Duration,
    max_frametime: Duration,
    elapsed: Duration,
    position: Position,
    execution_times: Option<&'a SecondaryMap<NodeIndex, (&'static str, Duration)>>,
}

impl<'a> Default for DebugReport<'a> {
    fn default() -> Self {
        Self {
            framerate: 0.0,
            min_frametime: Duration::from_secs(u64::MAX),
            avg_frametime: Duration::from_secs(0),
            max_frametime: Duration::from_secs(u64::MIN),
            elapsed: Duration::from_secs(0),
            position: Default::default(),
            execution_times: None,
        }
    }
}

impl<'a> Display for DebugReport<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:.2?}, {:.2?}, {:.2?}; {:.0?} fps\n{:.2?}\n{:#.2?}\n",
            self.min_frametime,
            self.avg_frametime,
            self.max_frametime,
            self.framerate,
            self.elapsed,
            self.position,
        )?;
        self.execution_times
            .map(|val| {
                val.iter()
                    .try_for_each(|(_, val)| write!(f, "{:?}: {}ms\n", val.0, val.1.ms()))
            })
            .transpose()?;

        Ok(())
    }
}

struct DebugLayer {
    elapsed: Clock,
    last_status: Clock,
    frequency: Duration,

    min: Duration,
    max: Duration,

    framecount: usize,
}

impl DebugLayer {
    fn new(
        _world: &mut World,
        _resources: &Resources,
        _events: &mut Events,
        frequency: Duration,
    ) -> anyhow::Result<Self> {
        log::debug!("Created debug layer");
        Ok(Self {
            elapsed: Clock::new(),
            last_status: Clock::new(),
            frequency,
            min: Duration::from_secs(u64::MAX),
            max: Duration::from_secs(u64::MIN),
            framecount: 0,
        })
    }
}

impl Layer for DebugLayer {
    fn on_update(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
        _: &mut Events,
        frametime: Duration,
    ) -> anyhow::Result<()> {
        let _scope = TimedScope::new(|elapsed| log::trace!("Debug layer took {:.3?}", elapsed));
        self.min = frametime.min(self.min);
        self.max = frametime.max(self.max);

        self.framecount += 1;

        let elapsed = self.last_status.elapsed();

        if elapsed > self.frequency {
            self.last_status.reset();

            let avg = Duration::div_f32(elapsed, self.framecount as f32);

            self.last_status.reset();

            let rendergraph = resources.get_default::<RenderGraph>()?;

            let report = DebugReport {
                framerate: 1.0 / avg.secs(),
                min_frametime: self.min,
                avg_frametime: avg,
                max_frametime: self.max,
                elapsed: self.elapsed.elapsed(),
                position: world
                    .query_mut::<(&Position, &MainCamera)>()
                    .into_iter()
                    .next()
                    .map(|(_, (p, _))| *p)
                    .unwrap_or_default(),

                execution_times: Some(rendergraph.execution_times()),
            };

            world
                .query_mut::<(&mut Text, &DisplayDebugReport)>()
                .into_iter()
                .for_each(|(_, (text, _))| {
                    let val = text.val_mut();
                    let val = val.to_mut();

                    val.clear();

                    write!(val, "{}", &report).expect("Failed to write into string");
                });

            log::debug!("{:?}", report.framerate);

            // Reset
            self.framecount = 0;
            self.min = Duration::from_secs(u64::MAX);
            self.max = Duration::from_secs(u64::MIN);
        }

        Ok(())
    }
}
