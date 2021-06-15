use anyhow::Context;
use atomic_refcell::AtomicRefCell;
use batched_mesh_renderer::BatchedMeshRenderer;
use components::{AngularVelocity, Position, Rotation, Scale};
use flume::Receiver;
use glfw::{Action, CursorMode, Glfw, Key, Window, WindowEvent};
use hecs::World;
use indirect_mesh_renderer::IndirectMeshRenderer;
use ivy_core::{resources::Handle, *};
use ivy_graphics::{
    window::{WindowExt, WindowInfo, WindowMode},
    Material, Mesh, ShaderPass,
};
use ivy_input::{Input, InputAxis, InputVector};
use ivy_ui::{Image, ImageRenderer};
use ivy_vulkan::{commands::*, descriptors::*, *};
use mesh_renderer::MeshRenderer;
use std::{
    sync::{mpsc, Arc},
    time::Duration,
};
use ultraviolet::{projection, Mat4, Rotor3, Vec2, Vec3, Vec4};

use log::*;
use window_renderer::WindowRenderer;

mod batched_mesh_renderer;
mod components;
mod indirect_mesh_renderer;
mod mesh_renderer;
mod systems;
mod window_renderer;

const FRAMES_IN_FLIGHT: usize = 2;

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
            extent: Some(Extent::new(800, 600)),

            resizable: false,
            mode: WindowMode::Windowed,
        },
    )?;

    let window = Arc::new(AtomicRefCell::new(window));

    let context = Arc::new(VulkanContext::new(&glfw, &window.borrow())?);

    let mut app = App::builder()
        .push_layer(|_, _| WindowLayer::new(glfw, window.clone(), window_events))
        .push_layer(|w, e| LogicLayer::new(w, e, window.clone()))
        .try_push_layer(|world, events| {
            VulkanLayer::new(context.clone(), world, window.clone(), events)
        })?
        .push_layer(|_, _| PerformanceLayer::new(1.secs()))
        .build();

    anyhow::Result::from(app.run()).context("Failed to run application")
}

struct Camera;

struct LogicLayer {
    window: Arc<AtomicRefCell<Window>>,
    input: Input,
    input_vec: InputVector,

    camera_vel: f32,
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

        world.spawn((
            Camera,
            Position(Vec3::new(0.0, 0.0, 5.0)),
            Rotation(Rotor3::identity()),
        ));

        let (tx, rx) = flume::unbounded();
        events.subscribe(tx);

        Self {
            window,
            input,
            camera_vel: 5.0,
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
            debug!("Event: {:?}", event);
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
                    self.camera_vel += scroll as f32;
                    self.camera_vel = self.camera_vel.clamp(0.0, 10.0);
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

            *camera_pos += Position(camera_rot.into_matrix() * (movement * dt * self.camera_vel));

            systems::integrate_angular_velocity(world, dt);

            systems::generate_model_matrices(world);

            self.acc -= self.timestep.secs();
        }

        Ok(())
    }
}

struct DiffusePass {
    pipeline: Pipeline,
}

impl DiffusePass {
    fn new(pipeline: Pipeline) -> Self {
        Self { pipeline }
    }
}

impl ShaderPass for DiffusePass {
    fn pipeline(&self) -> &Pipeline {
        &self.pipeline
    }

    fn pipeline_layout(&self) -> vk::PipelineLayout {
        self.pipeline.layout()
    }
}
#[derive(Copy, Clone, Debug)]
enum CurrentRenderer {
    Direct,
    Batched,
    Indirect,
}

#[allow(dead_code)]
struct VulkanLayer {
    context: Arc<VulkanContext>,

    window_renderer: WindowRenderer,
    window: Arc<AtomicRefCell<Window>>,
    mesh_renderer: MeshRenderer,
    image_renderer: ImageRenderer,
    batched_renderer: BatchedMeshRenderer,
    indirect_renderer: IndirectMeshRenderer,

    current_renderer: CurrentRenderer,

    descriptor_layout_cache: DescriptorLayoutCache,
    descriptor_allocator: DescriptorAllocator,

    frames: Vec<FrameData>,

    global_data: GlobalData,
    current_frame: usize,

    clock: Clock,
    materials: ResourceCache<Material>,
    textures: ResourceCache<Texture>,
    samplers: ResourceCache<Sampler>,
    diffuse_passes: ResourceCache<DiffusePass>,
    meshes: ResourceCache<Mesh>,
    images: ResourceCache<Image>,

    window_events: Receiver<WindowEvent>,

    cube_mesh: Handle<Mesh>,
    material: Handle<Material>,

    pass: Handle<DiffusePass>,
}

impl VulkanLayer {
    pub fn new(
        context: Arc<VulkanContext>,
        world: &mut World,
        window: Arc<AtomicRefCell<Window>>,
        events: &mut Events,
    ) -> anyhow::Result<Self> {
        let mut descriptor_layout_cache = DescriptorLayoutCache::new(context.device().clone());

        let mut descriptor_allocator = DescriptorAllocator::new(context.device().clone(), 2);

        let window_renderer = WindowRenderer::new(context.clone(), window.clone())?;
        let indirect_renderer =
            IndirectMeshRenderer::new(context.clone(), &mut descriptor_layout_cache, 16)?;
        let mesh_renderer = MeshRenderer::new(
            context.clone(),
            &mut descriptor_layout_cache,
            &mut descriptor_allocator,
        )?;
        let batched_renderer = BatchedMeshRenderer::new(
            context.clone(),
            &mut descriptor_layout_cache,
            &mut descriptor_allocator,
        )?;

        let image_renderer = ImageRenderer::new(
            context.clone(),
            &mut descriptor_layout_cache,
            16,
            FRAMES_IN_FLIGHT,
        )?;

        // Data that is tied and updated per swapchain image basis
        let frames = (0..FRAMES_IN_FLIGHT)
            .map(|_| {
                FrameData::new(
                    context.clone(),
                    &mut descriptor_allocator,
                    &mut descriptor_layout_cache,
                )
            })
            .collect::<Result<Vec<FrameData>, _>>()?;

        let mut meshes = ResourceCache::new();

        let document =
            ivy_graphics::Document::load(context.clone(), &mut meshes, "./res/models/cube.gltf")
                .context("Failed to load cube model")?;

        let cube_mesh = document.mesh(0);

        let document =
            ivy_graphics::Document::load(context.clone(), &mut meshes, "./res/models/sphere.gltf")
                .context("Failed to load sphere model")?;

        let sphere_mesh = document.mesh(0);

        let mut textures = ResourceCache::new();

        let grid = textures.insert(
            Texture::load(context.clone(), "./res/textures/grid.png")
                .context("Failed to load grid texture")?,
        );

        let uv_grid = textures.insert(
            Texture::load(context.clone(), "./res/textures/uv.png")
                .context("Failed to load uv texture")?,
        );

        let mut samplers = ResourceCache::new();

        let sampler = samplers.insert(Sampler::new(
            context.clone(),
            SamplerInfo {
                address_mode: AddressMode::REPEAT,
                mag_filter: FilterMode::LINEAR,
                min_filter: FilterMode::LINEAR,
                unnormalized_coordinates: false,
                anisotropy: 16.0,
                mip_levels: 4,
            },
        )?);

        let mut materials = ResourceCache::new();

        let material = materials.insert(Material::new(
            &context,
            &mut descriptor_layout_cache,
            &mut descriptor_allocator,
            &textures,
            &samplers,
            grid,
            sampler,
        )?);

        let material2 = materials.insert(Material::new(
            &context,
            &mut descriptor_layout_cache,
            &mut descriptor_allocator,
            &textures,
            &samplers,
            uv_grid,
            sampler,
        )?);

        let mut images = ResourceCache::new();

        let image: Handle<Image> = images.insert(Image::new(
            &context,
            &mut descriptor_layout_cache,
            &mut descriptor_allocator,
            &mut textures,
            &mut samplers,
            uv_grid,
            sampler,
        )?);


        let viewproj =
            ultraviolet::projection::perspective_vk(
                1.0,
                window.borrow().extent().aspect(),
                0.1,
                100.0,
            ) * ultraviolet::Mat4::look_at(Vec3::new(5.0, 0.5, 5.0), Vec3::zero(), Vec3::unit_y());

        // An example uniform containing global uniform data
        let global_data = GlobalData {
            color: Vec4::new(0.3, 0.0, 8.0, 1.0),
            viewproj,
        };

        // Create a pipeline from the shaders
        let pipeline = Pipeline::new(
            context.device().clone(),
            &mut descriptor_layout_cache,
            &window_renderer.renderpass(),
            PipelineInfo {
                vertexshader: "./res/shaders/default.vert.spv".into(),
                fragmentshader: "./res/shaders/default.frag.spv".into(),
                vertex_binding: Vertex::binding_description(),
                vertex_attributes: Vertex::attribute_descriptions(),
                samples: SampleCountFlags::TYPE_1,
                extent: window_renderer.swapchain().extent(),
                subpass: 0,
                polygon_mode: vk::PolygonMode::FILL,
                cull_mode: vk::CullModeFlags::NONE,
                front_face: vk::FrontFace::CLOCKWISE,
            },
        )?;

        // Create a pipeline from the shaders
        let uv_pipeline = Pipeline::new(
            context.device().clone(),
            &mut descriptor_layout_cache,
            &window_renderer.renderpass(),
            PipelineInfo {
                vertexshader: "./res/shaders/default.vert.spv".into(),
                fragmentshader: "./res/shaders/uv.frag.spv".into(),
                vertex_binding: Vertex::binding_description(),
                vertex_attributes: Vertex::attribute_descriptions(),
                samples: SampleCountFlags::TYPE_1,
                extent: window_renderer.swapchain().extent(),
                subpass: 0,
                polygon_mode: vk::PolygonMode::FILL,
                cull_mode: vk::CullModeFlags::NONE,
                front_face: vk::FrontFace::CLOCKWISE,
            },
        )?;

        let mut diffuse_passes = ResourceCache::new();
        let default_shaderpass = diffuse_passes.insert(DiffusePass::new(pipeline));
        let uv_shaderpass = diffuse_passes.insert(DiffusePass::new(uv_pipeline));

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
                        AngularVelocity(Vec3::new(0.0, y as f32 * 0.5, x as f32)),
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
            sphere_mesh.clone(),
            Rotation::default(),
            AngularVelocity(Vec3::new(0.0, 0.1, 1.0)),
            uv_shaderpass.clone(),
            material.clone(),
        ));

        world.spawn((
            Mat4::from_translation(Vec3::one()),
            image,
            default_shaderpass,
        ));

        let (tx, rx) = flume::unbounded();

        events.subscribe(tx);

        Ok(Self {
            context,
            window_renderer,
            window,
            mesh_renderer,
            batched_renderer,
            indirect_renderer,
            current_renderer: CurrentRenderer::Indirect,
            descriptor_layout_cache,
            descriptor_allocator,
            frames,
            global_data,
            current_frame: 0,
            clock: Clock::new(),
            materials,
            textures,
            samplers,
            diffuse_passes,
            meshes,
            window_events: rx,
            cube_mesh,
            material,
            pass: default_shaderpass,
            image_renderer,
            images,
        })
    }

    fn handle_events(&mut self) {
        for event in self.window_events.try_iter() {
            if let WindowEvent::Key(Key::Num1, _, glfw::Action::Press, _) = event {
                self.current_renderer = CurrentRenderer::Direct;
                info!("Current renderer: {:?}", self.current_renderer);
            } else if let WindowEvent::Key(Key::Num2, _, glfw::Action::Press, _) = event {
                self.current_renderer = CurrentRenderer::Batched;
                info!("Current renderer: {:?}", self.current_renderer);
            } else if let WindowEvent::Key(Key::Num3, _, glfw::Action::Press, _) = event {
                self.current_renderer = CurrentRenderer::Indirect;
                info!("Current renderer: {:?}", self.current_renderer);
            }
        }
    }
}

impl Layer for VulkanLayer {
    fn on_update(
        &mut self,
        world: &mut World,
        _events: &mut Events,
        _frame_time: Duration,
    ) -> anyhow::Result<()> {
        self.handle_events();

        let extent = self.window_renderer.swapchain().extent();

        let frame = &mut self.frames[self.current_frame];

        fence::wait(self.context.device(), &[frame.fence], true)?;
        fence::reset(self.context.device(), &[frame.fence])?;

        let (_e, (camera_pos, camera_rot)) = world
            .query_mut::<(&Position, &Rotation)>()
            .with::<Camera>()
            .into_iter()
            .next()
            .unwrap();

        frame.commandpool.reset(false)?;

        let cmd = &frame.commandbuffer;

        // Begin recording the commandbuffer, hinting that it will only be used once
        cmd.begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)?;

        // Begin surface rendering renderpass
        self.window_renderer.begin(cmd)?;

        let viewproj = projection::perspective_vk(1.0, extent.aspect(), 0.1, 100.0)
            // * Mat4::look_at(*self.camera_pos, Vec3::zero(), Vec3::unit_y());
            * (Mat4::from_translation(**camera_pos) * camera_rot.into_matrix().into_homogeneous()).inversed();

        self.global_data.viewproj = viewproj;

        // Update global uniform buffer
        frame.global_uniformbuffer.fill(0, &[self.global_data])?;

        match self.current_renderer {
            CurrentRenderer::Direct => self.mesh_renderer.draw::<DiffusePass>(
                world,
                cmd,
                self.current_frame,
                frame.set,
                &mut self.materials,
                &mut self.meshes,
                &mut self.diffuse_passes,
            )?,
            CurrentRenderer::Batched =>
            // Bind the global uniform buffer
            {
                self.batched_renderer.update(world, self.current_frame)?;

                self.batched_renderer.draw::<DiffusePass>(
                    world,
                    cmd,
                    self.current_frame,
                    frame.set,
                    &mut self.materials,
                    &mut self.meshes,
                    &mut self.diffuse_passes,
                )?
            }
            CurrentRenderer::Indirect => {
                self.indirect_renderer
                    .register_entities::<DiffusePass>(world, &mut self.descriptor_layout_cache)?;

                self.indirect_renderer.update(world, self.current_frame)?;

                // Bind the global uniform buffer
                self.indirect_renderer.draw::<DiffusePass>(
                    world,
                    cmd,
                    self.current_frame,
                    frame.set,
                    &mut self.materials,
                    &mut self.meshes,
                    &mut self.diffuse_passes,
                )?;
            }
        }

        // Draw ui
        self.image_renderer
            .register_entities::<DiffusePass>(world, &mut self.descriptor_layout_cache)?;

        self.image_renderer.update(world, self.current_frame)?;

        self.image_renderer.draw(
            world,
            cmd,
            self.current_frame,
            frame.set,
            &mut self.images,
            &mut self.diffuse_passes,
        )?;

        // Done
        frame.commandbuffer.end_renderpass();
        cmd.end()?;

        // Submit and present
        self.window_renderer.submit(cmd, frame.fence)?;
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

/// Represents data needed to be duplicated for each swapchain image
struct FrameData {
    context: Arc<VulkanContext>,
    fence: Fence,
    set: DescriptorSet,
    global_uniformbuffer: Buffer,
    commandpool: CommandPool,
    commandbuffer: CommandBuffer,
}

impl FrameData {
    fn new(
        context: Arc<VulkanContext>,
        descriptor_allocator: &mut DescriptorAllocator,
        descriptor_layout_cache: &mut DescriptorLayoutCache,
    ) -> Result<Self, ivy_vulkan::Error> {
        let global_uniformbuffer = Buffer::new(
            context.clone(),
            BufferType::Uniform,
            BufferAccess::MappedPersistent,
            &[GlobalData {
                color: Vec4::new(1.0, 0.0, 0.0, 1.0),
                viewproj: Mat4::identity(),
            }],
        )?;

        let set = DescriptorBuilder::new()
            .bind_uniform_buffer(0, vk::ShaderStageFlags::VERTEX, &global_uniformbuffer)
            .build_one(
                context.device(),
                descriptor_layout_cache,
                descriptor_allocator,
            )?;

        let commandpool = CommandPool::new(
            context.device().clone(),
            context.queue_families().graphics().unwrap(),
            true,
            false,
        )?;

        let commandbuffer = commandpool.allocate_one()?;
        let fence = fence::create(context.device(), true)?;

        Ok(FrameData {
            context,
            fence,
            set,
            global_uniformbuffer,
            commandpool,
            commandbuffer,
        })
    }
}

impl Drop for FrameData {
    fn drop(&mut self) {
        let device = self.context.device();

        fence::destroy(device, self.fence);
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

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    position: Vec3,
    normal: Vec3,
    texcoord: Vec2,
}

impl Vertex {
    pub fn new(position: Vec3, normal: Vec3, texcoord: Vec2) -> Self {
        Self {
            position,
            normal,
            texcoord,
        }
    }
}

const ATTRIBUTE_DESCRIPTIONS: &'static [vk::VertexInputAttributeDescription] = &[
    // vec3 3*4 bytes
    vk::VertexInputAttributeDescription {
        binding: 0,
        location: 0,
        format: vk::Format::R32G32B32_SFLOAT,
        offset: 0,
    },
    // vec3 3*4 bytes
    vk::VertexInputAttributeDescription {
        binding: 0,
        location: 1,
        format: vk::Format::R32G32B32_SFLOAT,
        offset: 12,
    },
    // vec2 2*4 bytes
    vk::VertexInputAttributeDescription {
        binding: 0,
        location: 2,
        format: vk::Format::R32G32_SFLOAT,
        offset: 12 + 12,
    },
];

impl VertexDesc for Vertex {
    fn binding_description() -> vk::VertexInputBindingDescription {
        vk::VertexInputBindingDescription {
            binding: 0,
            stride: std::mem::size_of::<Self>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        }
    }

    fn attribute_descriptions() -> &'static [vk::VertexInputAttributeDescription] {
        ATTRIBUTE_DESCRIPTIONS
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
struct GlobalData {
    color: Vec4,
    viewproj: Mat4,
}
