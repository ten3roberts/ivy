use std::{
    sync::{mpsc, Arc},
    thread::sleep,
    time::Duration,
};

use components::{AngularVelocity, Position, Rotation, Scale};
use glfw::{Glfw, Window, WindowEvent};
use hecs::World;
use ivy_core::*;
use ivy_graphics::{
    window::{WindowExt, WindowInfo, WindowMode},
    Mesh,
};
use ivy_vulkan::{commands::*, descriptors::*, *};
use mesh_renderer::MeshRenderer;
use ultraviolet::{Mat4, Rotor3, Vec2, Vec3, Vec4};

use log::*;
use window_renderer::WindowRenderer;

mod components;
mod mesh_renderer;
mod systems;
mod window_renderer;

const FRAMES_IN_FLIGHT: usize = 2;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Logger {
        show_location: true,
        max_level: LevelFilter::Debug,
    }
    .install();

    let mut glfw = glfw::init(glfw::FAIL_ON_ERRORS)?;

    let (window, events) = ivy_graphics::window::create(
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
        .push_layer(|_, _| WindowLayer::new(glfw, window.clone(), events))
        .try_push_layer(|world, _| VulkanLayer::new(context.clone(), world, window.clone()))?
        .push_layer(|_, _| PerformanceLayer::new(1.secs()))
        .build();

    app.run();

    Ok(())
}

#[allow(dead_code)]
struct VulkanLayer {
    context: Arc<VulkanContext>,

    window_renderer: WindowRenderer,
    mesh_renderer: MeshRenderer,

    descriptor_layout_cache: DescriptorLayoutCache,
    descriptor_allocator: DescriptorAllocator,
    pipeline: Arc<Pipeline>,

    frames: Vec<FrameData>,

    mesh: Arc<Mesh>,
    global_data: GlobalData,
    current_frame: usize,

    clock: Clock,
}

impl VulkanLayer {
    pub fn new(
        context: Arc<VulkanContext>,
        world: &mut World,
        window: Arc<glfw::Window>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        let mut descriptor_layout_cache = DescriptorLayoutCache::new(context.device().clone());

        let mut descriptor_allocator = DescriptorAllocator::new(context.device().clone(), 2);

        let window_renderer = WindowRenderer::new(context.clone(), window.clone())?;
        let mesh_renderer = MeshRenderer::new(
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

        let document = ivy_graphics::Document::load(context.clone(), "./res/models/cube.gltf")?;

        let mesh = document.mesh(0).clone();

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
        let pipeline = Arc::new(Pipeline::new(
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
        )?);

        world.spawn_batch((0..100).map(|i| (Vec3::new(i as f32, 0.0, 0.0),)));
        world.spawn_batch(
            [
                (
                    Position(Vec3::new(0.0, 0.0, 0.0)),
                    mesh.clone(),
                    pipeline.clone(),
                ),
                (
                    Position(Vec3::new(4.0, 0.0, 0.0)),
                    mesh.clone(),
                    pipeline.clone(),
                ),
                (
                    Position(Vec3::new(0.0, 0.0, -3.0)),
                    mesh.clone(),
                    pipeline.clone(),
                ),
            ]
            .iter()
            .cloned(),
        );

        world.spawn((
            Position(Vec3::new(1.0, -2.0, -3.0)),
            mesh.clone(),
            Rotation::default(),
            Scale(Vec3::one() * 0.5),
            pipeline.clone(),
        ));

        world.spawn((
            Position(Vec3::new(4.0, 0.0, -3.0)),
            mesh.clone(),
            Rotation::default(),
            AngularVelocity(Vec3::new(0.0, 1.0, 0.2)),
            pipeline.clone(),
        ));

        Ok(Self {
            context,
            window_renderer,
            mesh_renderer,
            descriptor_layout_cache,
            descriptor_allocator,
            pipeline,
            frames,
            mesh,
            global_data,
            current_frame: 0,
            clock: Clock::new(),
        })
    }
}

impl Layer for VulkanLayer {
    fn on_update(&mut self, world: &mut World, _events: &mut Events) {
        let extent = self.window_renderer.swapchain().extent();

        let frame = &mut self.frames[self.current_frame];

        fence::wait(self.context.device(), &[frame.fence], true).unwrap();
        fence::reset(self.context.device(), &[frame.fence]).unwrap();

        systems::generate_model_matrices(world);
        systems::integrate_angular_velocity(world, 0.02);
        sleep(5.ms());

        frame.commandpool.reset(false).unwrap();

        let cmd = &frame.commandbuffer;

        // Begin recording the commandbuffer, hinting that it will only be used once
        cmd.begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .unwrap();

        // Begin surface rendering renderpass
        self.window_renderer.begin(cmd).unwrap();

        let viewproj = ultraviolet::projection::perspective_vk(1.0, extent.aspect(), 0.1, 100.0)
            * ultraviolet::Mat4::look_at(
                Vec3::new(5.0, self.clock.elapsed().secs().sin() * 10.0, 5.0),
                Vec3::zero(),
                Vec3::unit_y(),
            );

        self.global_data.viewproj = viewproj;

        // Update global uniform buffer
        frame
            .global_uniformbuffer
            .fill(0, &[self.global_data])
            .unwrap();

        // Bind the global uniform buffer

        self.mesh_renderer
            .draw(world, cmd, self.current_frame, frame.set)
            .unwrap();

        // Done
        frame.commandbuffer.end_renderpass();
        cmd.end().unwrap();

        // Submit and present
        self.window_renderer.submit(cmd, frame.fence).unwrap();
    }
}

impl Drop for VulkanLayer {
    fn drop(&mut self) {
        let device = self.context.device();
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
    fn on_update(&mut self, _world: &mut World, events: &mut Events) {
        self.glfw.poll_events();

        for (_, event) in glfw::flush_messages(&self.events) {
            if let WindowEvent::Close = event {
                events.send(AppEvent::Exit);
            }

            events.send(event);
        }
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
    fn on_update(&mut self, _: &mut World, _: &mut Events) {
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
