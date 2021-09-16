use std::{
    ops::{Add, Mul},
    sync::{mpsc, Arc},
    time::Duration,
};

use anyhow::{anyhow, Context};
use atomic_refcell::AtomicRefCell;
use flume::Receiver;
use glfw::{Action, CursorMode, Glfw, Key, Window, WindowEvent};
use hecs::*;
use hecs_hierarchy::Hierarchy;
use ivy::{
    core::*,
    graphics::{
        window::{WindowExt, WindowInfo, WindowMode},
        *,
    },
    input::*,
    physics::components::{AngularVelocity, Velocity},
    postprocessing::pbr::{create_pbr_pipeline, PBRInfo},
    random::{
        rand::{prelude::StdRng, SeedableRng},
        Random,
    },
    rendergraph::*,
    resources::*,
    ui::{constraints::*, *},
    vulkan::vk::CullModeFlags,
    vulkan::*,
    *,
};
use ultraviolet::{Rotor3, Vec2, Vec3};

use log::*;

mod route;

const FRAMES_IN_FLIGHT: usize = 2;

struct SineWave<T> {
    amplitude: T,
    frequency: f32,
    base_value: T,
    elapsed: f32,
}

impl<T> SineWave<T>
where
    T: 'static + Send + Sync + Copy + Mul<f32, Output = T> + Add<T, Output = T>,
{
    fn new(amplitude: T, frequency: f32, base_value: T) -> Self {
        Self {
            amplitude,
            frequency,
            base_value,
            elapsed: 0.0,
        }
    }

    fn update(world: &mut World, dt: f32) {
        world
            .query::<(&mut SineWave<T>, &mut T)>()
            .iter()
            .for_each(|(_, (wave, val))| {
                wave.elapsed += dt;
                let current = (wave.elapsed * wave.frequency * std::f32::consts::TAU).sin();
                *val = wave.amplitude * current + wave.base_value;
            });
    }
}

fn main() -> anyhow::Result<()> {
    Logger {
        show_location: true,
        max_level: LevelFilter::Debug,
    }
    .install();

    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS)?;

    let (window, window_events) = ivy_graphics::window::create(
        &mut glfw,
        "ivy example-vulkan",
        WindowInfo {
            extent: None,

            resizable: false,
            mode: WindowMode::Windowed,
        },
    )?;

    let window = Arc::new(AtomicRefCell::new(window));

    let context = Arc::new(VulkanContext::new_with_window(&glfw, &window.borrow())?);

    let mut app = App::builder()
        .push_layer(|_, _| WindowLayer::new(glfw, window.clone(), window_events))
        .push_layer(|w, e| LogicLayer::new(w, e, window.clone()))
        .try_push_layer(|world, events| {
            VulkanLayer::new(context.clone(), world, window.clone(), events)
        })?
        .push_layer(|_, _| PerformanceLayer::new(1.secs()))
        .build();

    let result = app.run().context("Failed to run application");
    match result {
        Ok(()) => Ok(()),
        Err(err) => {
            log::error!("Encountered error: {:?}", err);
            Err(err)
        }
    }
}

struct LogicLayer {
    window: Arc<AtomicRefCell<Window>>,
    input: Input,
    input_vec: InputVector,

    cemra_speed: f32,
    camera_euler: Vec3,

    cursor_mode: CursorMode,

    acc: f32,
    timestep: Duration,

    window_events: Receiver<WindowEvent>,
}

impl LogicLayer {
    pub fn new(world: &mut World, events: &mut Events, window: Arc<AtomicRefCell<Window>>) -> Self {
        let input = Input::new(&window.borrow(), events);

        let input_vec = InputVector::new(
            InputAxis::keyboard(Key::D, Key::A),
            InputAxis::keyboard(Key::Space, Key::LeftControl),
            InputAxis::keyboard(Key::S, Key::W),
        );

        let extent = window.borrow().extent();

        world.spawn((
            Camera::perspective(1.0, extent.aspect(), 0.1, 100.0),
            Position(Vec3::new(0.0, 0.0, 5.0)),
            Rotation(Rotor3::identity()),
        ));

        world.spawn((
            Canvas,
            Size2D(extent.as_vec()),
            Position2D::new(0.0, 0.0),
            Camera::default(),
        ));

        let (tx, rx) = flume::unbounded();
        events.subscribe(tx);

        Self {
            window,
            input,
            cemra_speed: 5.0,
            camera_euler: Vec3::zero(),
            input_vec,
            timestep: 20.ms(),
            acc: 0.0,
            window_events: rx,
            cursor_mode: CursorMode::Normal,
        }
    }

    pub fn handle_events(&mut self) {
        for event in self.window_events.try_iter() {
            match event {
                WindowEvent::Key(Key::Escape, _, Action::Press, _) => {
                    if self.cursor_mode == CursorMode::Normal {
                        self.cursor_mode = CursorMode::Disabled;
                    } else {
                        self.cursor_mode = CursorMode::Normal;
                    }

                    self.window.borrow_mut().set_cursor_mode(self.cursor_mode);
                }
                WindowEvent::Focus(false) => {
                    self.cursor_mode = CursorMode::Normal;
                    self.window.borrow_mut().set_cursor_mode(self.cursor_mode)
                }
                WindowEvent::Focus(true) => {
                    self.cursor_mode = CursorMode::Disabled;
                    self.window.borrow_mut().set_cursor_mode(self.cursor_mode)
                }
                WindowEvent::Scroll(_, scroll) => {
                    self.cemra_speed += scroll as f32;
                    self.cemra_speed = self.cemra_speed.clamp(0.0, 20.0);
                }
                _ => {}
            }
        }
    }
}

impl Layer for LogicLayer {
    fn on_update(
        &mut self,
        world: &mut World,
        _: &mut Events,
        frame_time: Duration,
    ) -> anyhow::Result<()> {
        self.handle_events();

        self.input.on_update();

        self.acc += frame_time.secs();

        let dt = self.timestep.secs();

        {
            let (_e, camera_rot) = world
                .query_mut::<&mut Rotation>()
                .with::<Camera>()
                .into_iter()
                .next()
                .unwrap();

            let mouse_movement =
                self.input.rel_mouse_pos() / self.window.borrow().extent().as_vec();

            self.camera_euler += mouse_movement.xyz();

            *camera_rot = Rotor3::from_euler_angles(
                self.camera_euler.z,
                self.camera_euler.y,
                -self.camera_euler.x,
            )
            .into();
        }

        while self.acc > 0.0 {
            let (_e, (camera_pos, camera_rot)) = world
                .query_mut::<(&mut Position, &Rotation)>()
                .with::<Camera>()
                .into_iter()
                .next()
                .unwrap();

            let movement = self.input_vec.get(&self.input);

            *camera_pos += Position(camera_rot.into_matrix() * (movement * dt * self.cemra_speed));

            SineWave::<Position>::update(world, dt);

            graphics::systems::update_view_matrices(world);
            physics::systems::integrate_angular_velocity_system(world, dt);
            physics::systems::integrate_velocity_system(world, dt);

            ivy_graphics::systems::update_model_matrices(world);

            let canvas = world
                .query::<(&Canvas, &Camera)>()
                .iter()
                .next()
                .ok_or(anyhow!("Missing canvas"))?
                .0;

            ui::systems::statisfy_widgets(world);
            ui::systems::update_canvas(world, canvas)?;
            ui::systems::update(world)?;
            ui::systems::update_model_matrices(world);

            self.acc -= self.timestep.secs();
        }

        Ok(())
    }
}

new_shaderpass! {
    pub struct GeometryPass;
    pub struct WireframePass;
    pub struct UIPass;
    pub struct PostProcessingPass;
}

#[allow(dead_code)]
struct VulkanLayer {
    context: Arc<VulkanContext>,

    window: Arc<AtomicRefCell<Window>>,
    swapchain: Handle<Swapchain>,

    rendergraph: RenderGraph,

    clock: Clock,
    resources: Resources,

    window_events: Receiver<WindowEvent>,
}

fn setup_ui(
    world: &mut World,
    image: Handle<Image>,
    image2: Handle<Image>,
    ui_pass: Handle<UIPass>,
) -> anyhow::Result<()> {
    let canvas = world
        .query::<&Canvas>()
        .iter()
        .next()
        .ok_or(anyhow!("Missing canvas"))?
        .0;

    world.attach_new::<Widget, _>(
        canvas,
        (
            Widget,
            image,
            ui_pass,
            RelativeOffset::new(-0.25, -0.25),
            AbsoluteSize::new(100.0, 100.0),
        ),
    )?;

    let widget2 = world.attach_new::<Widget, _>(
        canvas,
        (
            Widget,
            image2,
            ui_pass,
            RelativeOffset::new(0.2, -0.1),
            RelativeSize(Vec2::new(0.5, 0.5)),
            Aspect::new(1.0),
        ),
    )?;

    world.attach_new::<Widget, _>(
        canvas,
        (
            Widget,
            image,
            ui_pass,
            RelativeOffset::new(0.3, 0.0),
            AbsoluteSize::new(200.0, 100.0),
            Aspect::new(1.0),
        ),
    )?;

    world.attach_new::<Widget, _>(
        widget2,
        (
            Widget,
            image2,
            ui_pass,
            RelativeSize::new(0.2, 0.2),
            AbsoluteOffset::new(10.0, 0.0),
            RelativeOffset::new(0.0, -1.0),
        ),
    )?;

    Ok(())
}

impl VulkanLayer {
    pub fn new(
        context: Arc<VulkanContext>,
        world: &mut World,
        window: Arc<AtomicRefCell<Window>>,
        events: &mut Events,
    ) -> anyhow::Result<Self> {
        let swapchain_info = ivy_vulkan::SwapchainInfo {
            present_mode: vk::PresentModeKHR::IMMEDIATE,
            image_count: FRAMES_IN_FLIGHT as _,
            ..Default::default()
        };

        let resources = Resources::new();

        let swapchain = resources.insert_default(Swapchain::new(
            context.clone(),
            &window.borrow(),
            swapchain_info,
        )?)?;

        let mut rendergraph = RenderGraph::new(context.clone(), FRAMES_IN_FLIGHT)?;

        resources.insert_default(IndirectMeshRenderer::new(
            context.clone(),
            16,
            FRAMES_IN_FLIGHT,
        )?)?;

        resources.insert_default(FullscreenRenderer)?;

        let camera = world
            .query::<&Camera>()
            .without::<Canvas>()
            .iter()
            .next()
            .unwrap()
            .0;

        let swapchain_extent = resources.get(swapchain)?.extent();

        let final_lit = resources.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent: swapchain_extent,
                mip_levels: 1,
                usage: ImageUsage::COLOR_ATTACHMENT
                    | ImageUsage::SAMPLED
                    | ImageUsage::TRANSFER_SRC,
                ..Default::default()
            },
        )?)?;

        let pbr_nodes =
            rendergraph.add_nodes(create_pbr_pipeline::<GeometryPass, PostProcessingPass>(
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

        let canvas = world
            .query::<(&Canvas, &Camera)>()
            .iter()
            .next()
            .context("No canvas found")?
            .0;

        resources.insert_default(ImageRenderer::new(context.clone(), 16, FRAMES_IN_FLIGHT)?)?;

        let ui_node = rendergraph.add_node(CameraNode::<UIPass, _>::new(
            canvas,
            resources.default::<ImageRenderer>()?,
            vec![AttachmentInfo {
                store_op: StoreOp::STORE,
                load_op: LoadOp::LOAD,
                initial_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                resource: final_lit,
            }],
            vec![],
            vec![],
            None,
            vec![ClearValue::Color(0.0, 0.0, 0.0, 0.0).into()],
        ));

        let geometry_node = pbr_nodes[0];
        let post_processing_node = pbr_nodes[1];

        let swapchain_node = rendergraph.add_node(SwapchainNode::new(
            context.clone(),
            swapchain,
            final_lit,
            vec![],
            &resources,
        )?);

        rendergraph.build(resources.fetch()?, swapchain_extent)?;

        assert!(rendergraph.node_renderpass(swapchain_node).is_err());

        let document = ivy_graphics::Document::load(
            context.clone(),
            resources.fetch_mut()?,
            "./res/models/cube.gltf",
        )
        .context("Failed to load cube model")?;

        let cube_mesh = document.mesh(0);

        let grid = resources.insert(
            Texture::load(context.clone(), "./res/textures/grid.png")
                .context("Failed to load grid texture")?,
        )?;

        let uv_grid = resources.insert(
            Texture::load(context.clone(), "./res/textures/uv.png")
                .context("Failed to load uv texture")?,
        )?;

        let font = Font::new(
            context.clone(),
            &resources,
            "./res/fonts/Lora/Lora-VariableFont_wght.ttf",
            &FontInfo {
                size: 58.0,
                ..Default::default()
            },
        )?;

        // let atlas = resources.insert(TextureAtlas::new(
        //     context.clone(),
        //     &resources,
        //     &TextureInfo {
        //         extent: Extent::new(128, 128),
        //         mip_levels: 1,
        //         usage: ImageUsage::SAMPLED | ImageUsage::TRANSFER_DST,
        //         format: Format::R8G8B8A8_SRGB,
        //         samples: SampleCountFlags::TYPE_1,
        //     },
        //     4,
        //     vec![
        //         ("red", image::Image::load("./res/textures/red.png", 4)?),
        //         ("green", image::Image::load("./res/textures/green.png", 4)?),
        //         ("blue", image::Image::load("./res/textures/blue.png", 4)?),
        //         ("heart", image::Image::load("./res/textures/heart.png", 4)?),
        //     ],
        // )?)?;

        let sampler = resources.insert(Sampler::new(
            context.clone(),
            SamplerInfo {
                address_mode: AddressMode::REPEAT,
                mag_filter: FilterMode::LINEAR,
                min_filter: FilterMode::LINEAR,
                unnormalized_coordinates: false,
                anisotropy: 16.0,
                mip_levels: 4,
            },
        )?)?;

        let ui_sampler = resources.insert(Sampler::new(
            context.clone(),
            SamplerInfo {
                address_mode: AddressMode::CLAMP_TO_EDGE,
                mag_filter: FilterMode::NEAREST,
                min_filter: FilterMode::NEAREST,
                unnormalized_coordinates: false,
                anisotropy: 16.0,
                mip_levels: 1,
            },
        )?)?;

        let material = resources.insert(Material::new(
            context.clone(),
            &resources,
            grid,
            sampler,
            0.3,
            0.4,
        )?)?;

        let material2 = resources.insert(Material::new(
            context.clone(),
            &resources,
            uv_grid,
            sampler,
            0.0,
            0.9,
        )?)?;

        let heart =
            resources.insert(Texture::load(context.clone(), "./res/textures/heart.png")?)?;

        let image = resources.insert(Image::new(&context, &resources, heart, ui_sampler)?)?;

        let image2 = resources.insert(Image::new(
            &context,
            &resources,
            font.atlas().texture(),
            ui_sampler,
        )?)?;

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

        let default_shaderpass = resources.insert(GeometryPass(pipeline))?;

        // Insert one default post processing pass
        resources.insert_default(PostProcessingPass(fullscreen_pipeline))?;

        world.spawn_batch(
            [
                (
                    Position(Vec3::new(0.0, 0.0, 0.0)),
                    cube_mesh,
                    material,
                    default_shaderpass,
                ),
                (
                    Position(Vec3::new(4.0, 0.0, 0.0)),
                    cube_mesh,
                    material,
                    default_shaderpass,
                ),
                (
                    Position(Vec3::new(0.0, 0.0, -3.0)),
                    cube_mesh,
                    material2,
                    default_shaderpass,
                ),
            ]
            .iter()
            .cloned(),
        );

        let mut rng = StdRng::seed_from_u64(42);

        world.spawn_batch((0..500).map(|_| {
            (
                Position(Vec3::rand_sphere(&mut rng) * 100.0),
                Rotation::default(),
                AngularVelocity(Vec3::rand_uniform(&mut rng)),
                Velocity(Vec3::rand_constrained_sphere(&mut rng, 0.5, 5.0)),
                cube_mesh,
                material,
                default_shaderpass,
            )
        }));

        world.spawn((
            Position(Vec3::new(7.0, 0.0, 0.0)),
            PointLight::new(1.0, Vec3::new(0.0, 0.0, 500.0)),
        ));

        world.spawn((
            Position(Vec3::new(0.0, 2.0, 5.0)),
            PointLight::new(0.3, Vec3::new(500.0, 0.0, 0.0)),
            SineWave::<Position>::new(
                Position(Vec3::unit_y() * 5.0),
                1.0 / 10.0,
                Position(Vec3::new(0.0, 2.0, 5.0)),
            ),
        ));

        // Create a pipeline from the shaders
        let ui_pipeline = Pipeline::new::<UIVertex>(
            context.clone(),
            &PipelineInfo {
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

        let ui_pass = resources.insert(UIPass(ui_pipeline))?;

        setup_ui(world, image, image2, ui_pass)?;

        let (tx, window_events) = flume::unbounded();
        events.subscribe(tx);

        Ok(Self {
            context,
            window,
            swapchain,
            rendergraph,
            clock: Clock::new(),
            resources,
            window_events,
        })
    }
}

impl Layer for VulkanLayer {
    fn on_update(
        &mut self,
        world: &mut World,
        _events: &mut Events,
        _frame_time: Duration,
    ) -> anyhow::Result<()> {
        // Ensure gpu side data for cameras
        GpuCameraData::create_gpu_cameras(self.context.clone(), world, FRAMES_IN_FLIGHT)?;

        let current_frame = self.rendergraph.begin()?;

        self.resources
            .get_mut(self.swapchain)?
            .acquire_next_image(self.rendergraph.wait_semaphore(current_frame))?;

        {
            self.resources
                .get_default_mut::<IndirectMeshRenderer>()?
                .update(world, current_frame)?;

            self.resources
                .get_default_mut::<ImageRenderer>()?
                .update(world, current_frame)?;
        }

        GpuCameraData::update_all_system(world, current_frame)?;
        LightManager::update_all_system(world, current_frame)?;

        self.rendergraph.execute(world, &self.resources)?;
        self.rendergraph.end()?;

        // // Present results
        self.resources.get(self.swapchain)?.present(
            self.context.present_queue(),
            &[self.rendergraph.signal_semaphore(current_frame)],
        )?;

        Ok(())
    }
}

impl Drop for VulkanLayer {
    fn drop(&mut self) {
        let device = self.context.device();
        // Wait for everything to be done before cleaning up
        device::wait_idle(device).unwrap();
    }
}

struct WindowLayer {
    glfw: Glfw,
    _window: Arc<AtomicRefCell<Window>>,
    events: mpsc::Receiver<(f64, WindowEvent)>,
}

impl WindowLayer {
    pub fn new(
        glfw: Glfw,
        window: Arc<AtomicRefCell<Window>>,
        events: mpsc::Receiver<(f64, WindowEvent)>,
    ) -> Self {
        Self {
            glfw,
            _window: window,
            events,
        }
    }
}

impl Layer for WindowLayer {
    fn on_update(
        &mut self,
        _world: &mut World,
        events: &mut Events,
        _frame_time: Duration,
    ) -> anyhow::Result<()> {
        self.glfw.poll_events();

        for (_, event) in glfw::flush_messages(&self.events) {
            if let WindowEvent::Close = event {
                events.send(AppEvent::Exit);
            }

            events.send(event);
        }

        Ok(())
    }
}

struct PerformanceLayer {
    elapsed: Clock,
    last_status: Clock,
    frequency: Duration,

    min: Duration,
    max: Duration,
    acc: Duration,

    framecount: usize,
}

impl PerformanceLayer {
    fn new(frequency: Duration) -> Self {
        Self {
            elapsed: Clock::new(),
            last_status: Clock::new(),
            frequency,
            min: std::u64::MAX.secs(),
            max: 0.secs(),
            acc: 0.secs(),
            framecount: 0,
        }
    }
}

impl Layer for PerformanceLayer {
    fn on_update(
        &mut self,
        _: &mut World,
        _: &mut Events,
        frame_time: Duration,
    ) -> anyhow::Result<()> {
        self.acc += frame_time;

        self.min = frame_time.min(self.min);
        self.max = frame_time.max(self.max);

        self.framecount += 1;

        if self.last_status.elapsed() > self.frequency {
            self.last_status.reset();

            let avg = self.acc / self.framecount as u32;

            info!(
                "Elapsed: {:?},\t Deltatime: {:?} {:?} {:?},\t Framerate: {}",
                self.elapsed.elapsed(),
                self.min,
                avg,
                self.max,
                1.0 / avg.secs()
            );

            self.min = std::u64::MAX.secs();
            self.max = 0.secs();
            self.acc = 0.secs();
            self.framecount = 0;
        }

        Ok(())
    }
}
