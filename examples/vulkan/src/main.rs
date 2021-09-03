use anyhow::{anyhow, Context};
use atomic_refcell::AtomicRefCell;
use flume::Receiver;
use glfw::{Action, CursorMode, Glfw, Key, Window, WindowEvent};
use hecs::World;
use hecs_hierarchy::Hierarchy;
use ivy_core::{App, AppEvent, Clock, Events, FromDuration, IntoDuration, Layer, Logger};
use ivy_graphics::{
    components::{AngularVelocity, Position, Rotation, Scale},
    new_shaderpass,
    window::{WindowExt, WindowInfo, WindowMode},
    Camera, FullscreenRenderer, GpuCameraData, IndirectMeshRenderer, Light, LightManager, Material,
    Vertex,
};
use ivy_input::{Input, InputAxis, InputVector};
use ivy_postprocessing::node::PostProcessingNode;
use ivy_postprocessing::pbr::PBRAttachments;
use ivy_rendergraph::{AttachmentInfo, CameraNode, RenderGraph, SwapchainNode};
use ivy_resources::{Handle, Resources};
use ivy_ui::{
    constraints::{AbsoluteOffset, AbsoluteSize, Aspect, RelativeOffset, RelativeSize},
    Canvas, Image, Position2D, Size2D, Widget,
};
use ivy_vulkan::{vk::CullModeFlags, *};
use std::{
    sync::{mpsc, Arc},
    time::Duration,
};
use ultraviolet::{Rotor3, Vec3};

use log::*;

use crate::route::Route2D;

mod route;

const FRAMES_IN_FLIGHT: usize = 3;

fn main() -> anyhow::Result<()> {
    Logger {
        show_location: true,
        max_level: LevelFilter::Debug,
    }
    .install();

    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS)?;

    let (window, window_events) = ivy_graphics::window::create(
        &mut glfw,
        "ivy-vulkan",
        WindowInfo {
            extent: None, //Some(Extent::new(800, 600)),

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

    app.run().context("Failed to run application")
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
            Camera::orthographic(100.0 * extent.aspect(), 100.0, 0.1, 1000.0),
            Position(Vec3::new(0.0, 0.0, 50.0)),
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

            route::update_routes(world, dt);

            ivy_graphics::systems::update_view_matrices(world);
            ivy_graphics::systems::integrate_angular_velocity(world, dt);

            ivy_graphics::systems::update_model_matrices(world);
            ivy_ui::systems::update_model_matrices(world);

            self.acc -= self.timestep.secs();
        }

        Ok(())
    }
}

new_shaderpass! {
    pub struct DiffusePass;
        pub struct WireframePass;
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
    diffuse_pass: Handle<DiffusePass>,
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
            diffuse_pass,
            RelativeOffset::new(-0.25, -0.25),
            AbsoluteSize::new(50.0, 50.0),
        ),
    )?;

    let widget2 = world.attach_new::<Widget, _>(
        canvas,
        (
            Widget,
            image,
            diffuse_pass,
            RelativeOffset::new(0.3, -0.5),
            AbsoluteSize::new(200.0, 100.0),
            Aspect::new(1.0),
            Route2D::new(
                vec![
                    RelativeOffset::new(0.0, 0.0),
                    RelativeOffset::new(0.5, 0.5),
                    RelativeOffset::new(-0.5, 0.5),
                ],
                0.1,
            ),
        ),
    )?;

    world.attach_new::<Widget, _>(
        canvas,
        (
            Widget,
            image,
            diffuse_pass,
            RelativeOffset::new(0.3, -0.5),
            AbsoluteSize::new(200.0, 100.0),
            Aspect::new(1.0),
            Route2D::new(
                vec![
                    RelativeOffset::new(0.0, 0.0),
                    RelativeOffset::new(0.5, 0.5),
                    RelativeOffset::new(-0.5, 0.5),
                ],
                0.2,
            ),
        ),
    )?;

    world.attach_new::<Widget, _>(
        widget2,
        (
            Widget,
            image2,
            diffuse_pass,
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

        resources.insert_default(LightManager::new(
            context.clone(),
            10,
            Vec3::one() * 0.01,
            FRAMES_IN_FLIGHT,
        )?)?;

        let mut rendergraph = RenderGraph::new(context.clone(), FRAMES_IN_FLIGHT)?;

        resources.insert_default(IndirectMeshRenderer::new(
            context.clone(),
            16,
            FRAMES_IN_FLIGHT,
        )?)?;

        resources.insert_default(FullscreenRenderer)?;

        GpuCameraData::create_gpu_cameras(context.clone(), world, FRAMES_IN_FLIGHT)?;

        let camera = world
            .query::<&Camera>()
            .without::<Canvas>()
            .iter()
            .next()
            .unwrap()
            .0;

        let swapchain_extent = resources.get(swapchain)?.extent();

        let depth_buffer = resources.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent: swapchain_extent,
                mip_levels: 1,
                usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT,
                format: Format::D32_SFLOAT,
                samples: SampleCountFlags::TYPE_1,
            },
        )?)?;

        let final_diffuse = resources.insert(Texture::new(
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

        let pbr_attachments = PBRAttachments::new(context.clone(), &resources, swapchain_extent)?;

        let camera_data = world.get::<GpuCameraData>(camera)?;
        let light_manager = resources.get_default::<LightManager>()?;

        // let fullscreen_sets = DescriptorBuilder::from_mutliple_resources(
        //     &context,
        //     &[
        //         (&InputAttachment(albedo_view), ShaderStageFlags::FRAGMENT),
        //         (&InputAttachment(position_view), ShaderStageFlags::FRAGMENT),
        //         (&InputAttachment(normal_view), ShaderStageFlags::FRAGMENT),
        //         (&InputAttachment(mr_view), ShaderStageFlags::FRAGMENT),
        //         (&camera_data.buffers(), ShaderStageFlags::FRAGMENT),
        //         (&light_manager.scene_buffers(), ShaderStageFlags::FRAGMENT),
        //         (&light_manager.light_buffers(), ShaderStageFlags::FRAGMENT),
        //     ],
        //     FRAMES_IN_FLIGHT,
        // )?;

        let diffuse_node =
            rendergraph.add_node(CameraNode::<DiffusePass, IndirectMeshRenderer>::new(
                camera,
                resources.default::<IndirectMeshRenderer>()?,
                vec![
                    AttachmentInfo {
                        final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        initial_layout: ImageLayout::UNDEFINED,
                        resource: pbr_attachments.albedo,
                        store_op: StoreOp::STORE,
                        load_op: LoadOp::CLEAR,
                    },
                    AttachmentInfo {
                        final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        initial_layout: ImageLayout::UNDEFINED,
                        resource: pbr_attachments.position,
                        store_op: StoreOp::STORE,
                        load_op: LoadOp::CLEAR,
                    },
                    AttachmentInfo {
                        final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        initial_layout: ImageLayout::UNDEFINED,
                        resource: pbr_attachments.normal,
                        store_op: StoreOp::STORE,
                        load_op: LoadOp::CLEAR,
                    },
                    AttachmentInfo {
                        final_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        initial_layout: ImageLayout::UNDEFINED,
                        resource: pbr_attachments.roughness_metallic,
                        store_op: StoreOp::STORE,
                        load_op: LoadOp::CLEAR,
                    },
                ],
                vec![],
                vec![],
                Some(AttachmentInfo {
                    initial_layout: ImageLayout::UNDEFINED,
                    final_layout: ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                    resource: depth_buffer,
                    store_op: StoreOp::DONT_CARE,
                    load_op: LoadOp::CLEAR,
                }),
                vec![
                    ClearValue::Color(0.0, 0.0, 0.0, 1.0).into(),
                    ClearValue::Color(0.0, 0.0, 0.0, 1.0).into(),
                    ClearValue::Color(0.0, 0.0, 0.0, 1.0).into(),
                    ClearValue::Color(0.0, 0.0, 0.0, 1.0).into(),
                    ClearValue::DepthStencil(1.0, 0).into(),
                ],
            ));

        let fullscreen_node = rendergraph.add_node(PostProcessingNode::<PostProcessingPass>::new(
            context.clone(),
            &resources,
            &[],
            &pbr_attachments.as_slice(),
            &[
                &camera_data.buffers(),
                &light_manager.scene_buffers(),
                &light_manager.light_buffers(),
            ],
            &[final_diffuse],
            FRAMES_IN_FLIGHT,
        )?);

        drop(camera_data);
        drop(light_manager);

        let swapchain_node = rendergraph.add_node(SwapchainNode::new(
            context.clone(),
            swapchain,
            final_diffuse,
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

        let document = ivy_graphics::Document::load(
            context.clone(),
            resources.fetch_mut()?,
            "./res/models/sphere.gltf",
        )
        .context("Failed to load sphere model")?;

        let sphere_mesh = document.mesh(0);

        let grid = resources.insert(
            Texture::load(context.clone(), "./res/textures/grid.png")
                .context("Failed to load grid texture")?,
        )?;

        let uv_grid = resources.insert(
            Texture::load(context.clone(), "./res/textures/uv.png")
                .context("Failed to load uv texture")?,
        )?;

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

        let image: Handle<Image> =
            resources.insert(Image::new(&context, &resources, uv_grid, sampler)?)?;

        let image2: Handle<Image> =
            resources.insert(Image::new(&context, &resources, grid, sampler)?)?;

        let fullscreen_pipeline = Pipeline::new::<()>(
            context.clone(),
            &PipelineInfo {
                vertexshader: "./res/shaders/fullscreen.vert.spv".into(),
                fragmentshader: "./res/shaders/post_processing.frag.spv".into(),
                samples: SampleCountFlags::TYPE_1,
                extent: swapchain_extent,
                cull_mode: CullModeFlags::NONE,
                ..rendergraph.pipeline_info(fullscreen_node)?
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
                polygon_mode: vk::PolygonMode::FILL,
                cull_mode: vk::CullModeFlags::NONE,
                front_face: vk::FrontFace::CLOCKWISE,
                ..rendergraph.pipeline_info(diffuse_node)?
            },
        )?;

        let default_shaderpass = resources.insert(DiffusePass(pipeline))?;

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

        let cube_side = 10;
        world.spawn_batch(
            (0..cube_side)
                .flat_map(move |x| (0..cube_side).map(move |y| (x, y)))
                .flat_map(move |(x, y)| (0..cube_side).map(move |z| (x, y, z)))
                .map(|(x, y, z)| {
                    (
                        cube_mesh,
                        Position(Vec3::new(
                            x as f32 * 3.0 - 5.0,
                            y as f32 * 3.0,
                            -z as f32 * 3.0,
                        )),
                        material,
                        default_shaderpass,
                        // Scale(Vec3::new(0.1, 0.1, 0.1)),
                        Rotation(Rotor3::identity()),
                        AngularVelocity(Vec3::new(0.0, y as f32 * 0.1, x as f32 * 0.1)),
                    )
                }),
        );

        world.spawn((
            Position(Vec3::new(1.0, -2.0, 3.0)),
            cube_mesh,
            Rotation::default(),
            Scale(Vec3::one() * 0.5),
            default_shaderpass,
            material2,
        ));

        world.spawn((
            Position(Vec3::new(0.0, 0.0, 3.0)),
            sphere_mesh,
            Rotation::default(),
            AngularVelocity(Vec3::new(0.0, 0.1, 1.0)),
            material,
        ));

        // world.spawn((
        //     Position(Vec3::new(3.0, 5.0, 7.0)),
        //     Light::new(Vec3::new(100.0, 100.0, 100.0)),
        // ));

        world.spawn((
            Position(Vec3::new(7.0, 0.0, 0.0)),
            Light::new(Vec3::new(0.0, 0.0, 20.0)),
        ));

        world.spawn((
            Position(Vec3::new(0.0, 2.0, 5.0)),
            Light::new(Vec3::new(20.0, 0.0, 0.0)),
        ));

        setup_ui(world, image, image2, default_shaderpass)?;

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
            let mut indirect_renderer = self.resources.get_default_mut::<IndirectMeshRenderer>()?;
            indirect_renderer.register_entities::<DiffusePass>(world)?;

            indirect_renderer.update(world, current_frame)?;
        }

        GpuCameraData::update_all(world, current_frame)?;

        self.resources.get_default_mut::<LightManager>()?.update(
            world,
            Vec3::zero(),
            current_frame,
        )?;

        self.rendergraph.execute(world, &self.resources)?;
        self.rendergraph.end()?;

        // Present results
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
        log::info!("Dropping vulkan layer");
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
