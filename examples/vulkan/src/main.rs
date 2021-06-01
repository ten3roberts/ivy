use std::{
    sync::{mpsc, Arc},
    time::Duration,
};

use anyhow::Context;

use batched_mesh_renderer::BatchedMeshRenderer;
use components::{AngularVelocity, Position, Rotation, Scale};
use flume::Receiver;
use glfw::{Glfw, Key, Window, WindowEvent};
use hecs::World;
use ivy_core::*;
use ivy_graphics::{
    window::{WindowExt, WindowInfo, WindowMode},
    Material, Mesh, ShaderPass,
};
use ivy_input::{Input, InputAxis, InputVector};
use ivy_vulkan::{commands::*, descriptors::*, *};
// use mesh_renderer::MeshRenderer;
use ultraviolet::{projection, Mat4, Rotor3, Vec2, Vec3, Vec4};

use log::*;
use window_renderer::WindowRenderer;

mod batched_mesh_renderer;
mod components;
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

    let window = Arc::new(window);

    let context = Arc::new(VulkanContext::new(&glfw, &window)?);

    let mut app = App::builder()
        .push_layer(|_, _| WindowLayer::new(glfw, window.clone(), window_events))
        .push_layer(|w, e| LogicLayer::new(w, e))
        .try_push_layer(|world, events| {
            VulkanLayer::new(context.clone(), world, window.clone(), events)
        })?
        .push_layer(|_, _| PerformanceLayer::new(1.secs()))
        .build();

    anyhow::Result::from(app.run()).context("Failed to run application")
}

struct Camera;

struct LogicLayer {
    input: Input,
    input_vec: InputVector,

    camera_vel: f32,
    frame_clock: Clock,

    rx: Receiver<WindowEvent>,
    acc: f32,
    timestep: Duration,
}

impl LogicLayer {
    pub fn new(world: &mut World, events: &mut Events) -> Self {
        let input = Input::new(events);

        let input_vec = InputVector::new(
            InputAxis::keyboard(Key::D, Key::A),
            InputAxis::keyboard(Key::Space, Key::LeftControl),
            InputAxis::keyboard(Key::S, Key::W),
        );

        let frame_clock = Clock::new();

        world.spawn((Camera, Position(Vec3::new(0.0, 0.0, 5.0))));
        let (tx, rx) = flume::unbounded();
        events.subscribe(tx);

        Self {
            input,
            camera_vel: 5.0,
            input_vec,
            frame_clock,
            rx,
            timestep: 20.ms(),
            acc: 0.0,
        }
    }
}

impl Layer for LogicLayer {
    fn on_update(&mut self, world: &mut World, _: &mut Events) -> anyhow::Result<()> {
        let frame_time = self.frame_clock.reset().secs();
        self.acc += frame_time;

        let dt = self.timestep.secs();

        while self.acc > 0.0 {
            self.input.on_update();

            let (_e, camera_pos) = world
                .query_mut::<&mut Position>()
                .with::<Camera>()
                .into_iter()
                .next()
                .unwrap();

            let movement = self.input_vec.get(&self.input);

            *camera_pos += Position(movement * dt * self.camera_vel);

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

#[allow(dead_code)]
struct VulkanLayer {
    context: Arc<VulkanContext>,

    window_renderer: WindowRenderer,
    window: Arc<Window>,
    mesh_renderer: BatchedMeshRenderer,

    descriptor_layout_cache: DescriptorLayoutCache,
    descriptor_allocator: DescriptorAllocator,

    frames: Vec<FrameData>,

    global_data: GlobalData,
    current_frame: usize,

    clock: Clock,
    materials: ResourceCache<Material>,
    diffuse_passes: ResourceCache<DiffusePass>,
    meshes: ResourceCache<Mesh>,
}

impl VulkanLayer {
    pub fn new(
        context: Arc<VulkanContext>,
        world: &mut World,
        window: Arc<Window>,
        _: &mut Events,
    ) -> anyhow::Result<Self> {
        let mut descriptor_layout_cache = DescriptorLayoutCache::new(context.device().clone());

        let mut descriptor_allocator = DescriptorAllocator::new(context.device().clone(), 2);

        let window_renderer = WindowRenderer::new(context.clone(), window.clone())?;
        let mesh_renderer = BatchedMeshRenderer::new(
            context.clone(),
            &mut descriptor_layout_cache,
            &mut descriptor_allocator,
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

        let grid = Arc::new(
            Texture::load(context.clone(), "./res/textures/grid.png")
                .context("Failed to load grid texture")?,
        );
        let uv_grid = Arc::new(
            Texture::load(context.clone(), "./res/textures/uv.png")
                .context("Failed to load uv texture")?,
        );

        let sampler = Arc::new(Sampler::new(
            context.clone(),
            SamplerInfo {
                address_mode: AddressMode::REPEAT,
                mag_filter: FilterMode::LINEAR,
                min_filter: FilterMode::LINEAR,
                unnormalized_coordinates: false,
                anisotropy: 16.0,
                mip_levels: grid.mip_levels(),
            },
        )?);

        let sampler2 = Arc::new(Sampler::new(
            context.clone(),
            SamplerInfo {
                address_mode: AddressMode::REPEAT,
                mag_filter: FilterMode::LINEAR,
                min_filter: FilterMode::LINEAR,
                unnormalized_coordinates: false,
                anisotropy: 16.0,
                mip_levels: uv_grid.mip_levels(),
            },
        )?);

        let mut materials = ResourceCache::new();

        let material = materials.insert(Material::new(
            context.clone(),
            &mut descriptor_layout_cache,
            &mut descriptor_allocator,
            grid,
            sampler,
        )?);

        let material2 = materials.insert(Material::new(
            context.clone(),
            &mut descriptor_layout_cache,
            &mut descriptor_allocator,
            uv_grid,
            sampler2,
        )?);

        let viewproj =
            ultraviolet::projection::perspective_vk(1.0, window.extent().aspect(), 0.1, 100.0)
                * ultraviolet::Mat4::look_at(
                    Vec3::new(5.0, 0.5, 5.0),
                    Vec3::zero(),
                    Vec3::unit_y(),
                );

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
        let cube_side = 15;
        world.spawn_batch(
            (0..cube_side)
                .flat_map(move |x| (0..cube_side).map(move |y| (x, y)))
                .flat_map(move |(x, y)| (0..cube_side).map(move |z| (x, y, z)))
                .map(|(x, y, z)| {
                    (
                        sphere_mesh,
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

        Ok(Self {
            context,
            window_renderer,
            window,
            mesh_renderer,
            descriptor_layout_cache,
            descriptor_allocator,
            frames,
            global_data,
            current_frame: 0,
            clock: Clock::new(),

            diffuse_passes,
            materials,
            meshes,
        })
    }
}

impl Layer for VulkanLayer {
    fn on_update(&mut self, world: &mut World, _events: &mut Events) -> anyhow::Result<()> {
        let extent = self.window_renderer.swapchain().extent();

        let frame = &mut self.frames[self.current_frame];

        fence::wait(self.context.device(), &[frame.fence], true)?;
        fence::reset(self.context.device(), &[frame.fence])?;

        let (_e, camera_pos) = world
            .query_mut::<&mut Position>()
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
            * Mat4::from_translation(**camera_pos).inversed();

        self.global_data.viewproj = viewproj;

        // Update global uniform buffer
        frame.global_uniformbuffer.fill(0, &[self.global_data])?;

        self.mesh_renderer.update(world, self.current_frame)?;

        // Bind the global uniform buffer
        self.mesh_renderer.draw::<DiffusePass>(
            world,
            cmd,
            self.current_frame,
            frame.set,
            &mut self.materials,
            &mut self.meshes,
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

        let mut set = DescriptorSet::null();
        DescriptorBuilder::new()
            .bind_uniform_buffer(0, vk::ShaderStageFlags::VERTEX, &global_uniformbuffer)
            .build(
                context.device(),
                descriptor_layout_cache,
                descriptor_allocator,
                &mut set,
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
    _window: Arc<Window>,
    events: mpsc::Receiver<(f64, WindowEvent)>,
}

impl WindowLayer {
    pub fn new(
        glfw: Glfw,
        window: Arc<Window>,
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
    fn on_update(&mut self, _world: &mut World, events: &mut Events) -> anyhow::Result<()> {
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
    clock: Clock,
    frame_clock: Clock,
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
            clock: Clock::new(),
            frame_clock: Clock::new(),
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
    fn on_update(&mut self, _: &mut World, _: &mut Events) -> anyhow::Result<()> {
        let dt = self.frame_clock.reset();

        self.acc += dt;

        self.min = dt.min(self.min);
        self.max = dt.max(self.max);

        self.framecount += 1;

        if self.last_status.elapsed() > self.frequency {
            self.last_status.reset();

            let avg = self.acc / self.framecount as u32;

            info!(
                "Elapsed: {:?},\t Deltatime: {:?} {:?} {:?},\t Framerate: {}",
                self.clock.elapsed(),
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
