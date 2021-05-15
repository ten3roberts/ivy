use std::{
    rc::Rc,
    sync::{mpsc, Arc},
    thread::sleep,
};

use glfw::{Glfw, Window, WindowEvent};
use hecs::World;
use ivy_core::*;
use ivy_graphics::{
    window::{WindowExt, WindowInfo, WindowMode},
    Mesh,
};
use ivy_vulkan::descriptors::*;
use ivy_vulkan::*;
use ivy_vulkan::{commands::*, vk::Semaphore};
use ultraviolet::{Mat4, Vec2, Vec3, Vec4};

use log::*;

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

    let vulkan_layer = VulkanLayer::new(&glfw, &window)?;
    let window_layer = WindowLayer::new(glfw, window, events);

    let mut app = App::builder()
        .push_layer(window_layer)
        .push_layer(vulkan_layer)
        .build();

    app.run();

    Ok(())
}

#[allow(dead_code)]
struct VulkanLayer {
    context: Rc<VulkanContext>,

    swapchain: Swapchain,
    depth_attachment: Texture,
    renderpass: RenderPass,
    descriptor_layout_cache: DescriptorLayoutCache,
    descriptor_allocator: DescriptorAllocator,
    pipeline: Pipeline,

    frames: Vec<FrameData>,
    framebuffers: Vec<Framebuffer>,

    present_semaphore: Semaphore,
    render_semaphore: Semaphore,

    mesh: Arc<Mesh>,
    global_data: GlobalData,
    current_frame: usize,

    clock: Clock,
}

impl VulkanLayer {
    pub fn new(glfw: &Glfw, window: &glfw::Window) -> Result<Self, Box<dyn std::error::Error>> {
        let context = Rc::new(VulkanContext::new(&glfw, &window)?);

        let swapchain = Swapchain::new(context.clone(), &window)?;
        debug!("Swapchain images: {}", swapchain.images().len());

        let extent = swapchain.extent();

        // Depth buffer attachment
        let depth_attachment = Texture::new(
            context.clone(),
            TextureInfo {
                extent: swapchain.extent(),
                mip_levels: 1,
                usage: TextureUsage::DepthAttachment,
                format: Format::D32_SFLOAT,
                samples: SampleCountFlags::TYPE_1,
            },
        )?;

        // Create a simple renderpass rendering directly to the swapchain images
        let renderpass_info = RenderPassInfo {
            attachments: &[
                AttachmentInfo::from_texture(
                    swapchain.image(0),
                    LoadOp::CLEAR,
                    StoreOp::STORE,
                    ImageLayout::UNDEFINED,
                    ImageLayout::PRESENT_SRC_KHR,
                ),
                AttachmentInfo::from_texture(
                    &depth_attachment,
                    LoadOp::CLEAR,
                    StoreOp::DONT_CARE,
                    ImageLayout::UNDEFINED,
                    ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                ),
            ],
            subpasses: &[SubpassInfo {
                color_attachments: &[AttachmentReference {
                    attachment: 0,
                    layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                }],
                resolve_attachments: &[],
                depth_attachment: Some(AttachmentReference {
                    attachment: 1,
                    layout: ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                }),
            }],
        };

        let renderpass = RenderPass::new(context.device_ref(), &renderpass_info)?;

        let framebuffers = swapchain
            .images()
            .iter()
            .map(|image| {
                Framebuffer::new(
                    context.device_ref(),
                    &renderpass,
                    &[image, &depth_attachment],
                    extent,
                )
            })
            .collect::<Result<Vec<_>, _>>()?;

        let mut descriptor_layout_cache = DescriptorLayoutCache::new(context.device_ref());

        let mut descriptor_allocator = DescriptorAllocator::new(context.device_ref(), 2);

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
        let pipeline = Pipeline::new(
            context.device_ref(),
            &mut descriptor_layout_cache,
            &renderpass,
            PipelineInfo {
                vertexshader: "./res/shaders/default.vert.spv".into(),
                fragmentshader: "./res/shaders/default.frag.spv".into(),
                vertex_binding: Vertex::binding_description(),
                vertex_attributes: Vertex::attribute_descriptions(),
                samples: SampleCountFlags::TYPE_1,
                extent: swapchain.extent(),
                subpass: 0,
                polygon_mode: vk::PolygonMode::FILL,
                cull_mode: vk::CullModeFlags::NONE,
                front_face: vk::FrontFace::CLOCKWISE,
            },
        )?;

        let render_semaphore = semaphore::create(context.device())?;
        let present_semaphore = semaphore::create(context.device())?;

        Ok(Self {
            context,
            swapchain,
            depth_attachment,
            renderpass,
            descriptor_layout_cache,
            descriptor_allocator,
            pipeline,
            frames,
            framebuffers,
            present_semaphore,
            render_semaphore,
            mesh,
            global_data,
            current_frame: 0,
            clock: Clock::new(),
        })
    }
}

impl Layer for VulkanLayer {
    fn on_update(&mut self, _world: &mut World, _events: &mut Events) {
        sleep(100.ms());

        let image_index = self.swapchain.next_image(self.present_semaphore).unwrap();

        debug!(
            "Image index: {},\t current_frame: {}",
            image_index, self.current_frame
        );

        let frame = &mut self.frames[self.current_frame];
        let framebuffer = &self.framebuffers[image_index as usize];

        fence::wait(self.context.device(), &[frame.fence], true).unwrap();
        fence::reset(self.context.device(), &[frame.fence]).unwrap();

        frame.commandpool.reset(false).unwrap();

        let viewproj = ultraviolet::projection::perspective_vk(
            1.0,
            self.swapchain.extent().aspect(),
            0.1,
            100.0,
        ) * ultraviolet::Mat4::look_at(
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

        // Begin the commandbuffer, hinting that it will only be used once
        frame
            .commandbuffer
            .begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)
            .unwrap();

        frame.commandbuffer.begin_renderpass(
            &self.renderpass,
            &framebuffer,
            self.swapchain.extent(),
            &[
                ClearValue::Color(0.0, 0.0, 0.0, 0.0),
                ClearValue::DepthStencil(1.0, 0),
            ],
        );

        // Bind the global uniform buffer
        frame
            .commandbuffer
            .bind_descriptor_sets(self.pipeline.layout(), 0, &[frame.set]);

        // Bind the pipeline
        frame.commandbuffer.bind_pipeline(&self.pipeline);

        // Bind and draw the triangle without indexing
        frame
            .commandbuffer
            .bind_vertexbuffers(0, &[&self.mesh.vertex_buffer()]);
        frame
            .commandbuffer
            .bind_indexbuffer(self.mesh.index_buffer(), 0);

        frame
            .commandbuffer
            .draw_indexed(self.mesh.index_count(), 1, 0, 0, 0);

        // Done
        frame.commandbuffer.end_renderpass();
        frame.commandbuffer.end().unwrap();

        // Which synchronization primities to wait on
        let wait_semaphores = [self.present_semaphore];

        let signal_semaphores = [self.render_semaphore];

        // Submit command buffers and signal fence `current_frame` when done
        frame
            .commandbuffer
            .submit(
                self.context.graphics_queue(),
                &wait_semaphores,
                &signal_semaphores,
                frame.fence,
                &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT],
            )
            .unwrap();

        self.swapchain
            .present(
                self.context.present_queue(),
                &signal_semaphores,
                image_index,
            )
            .unwrap();

        self.current_frame = (self.current_frame + 1) % FRAMES_IN_FLIGHT;
    }

    fn on_attach(&mut self, _world: &mut World, _events: &mut Events) {}
}

impl Drop for VulkanLayer {
    fn drop(&mut self) {
        let device = self.context.device();
        // Wait for everything to be done before cleaning up
        device::wait_idle(device).unwrap();

        semaphore::destroy(device, self.present_semaphore);
        semaphore::destroy(device, self.render_semaphore);
    }
}

/// Represents data needed to be duplicated for each swapchain image
struct FrameData {
    context: Rc<VulkanContext>,

    fence: Fence,

    commandpool: CommandPool,
    commandbuffer: CommandBuffer,

    set: DescriptorSet,
    global_uniformbuffer: Buffer,
}

impl FrameData {
    fn new(
        context: Rc<VulkanContext>,
        descriptor_allocator: &mut DescriptorAllocator,
        descriptor_layout_cache: &mut DescriptorLayoutCache,
    ) -> Result<Self, ivy_vulkan::Error> {
        let fence = fence::create(context.device(), true)?;

        // Create and record command buffers
        let commandpool = CommandPool::new(
            context.device_ref(),
            context.queue_families().graphics().unwrap(),
            true,
            false,
        )?;

        let commandbuffer = commandpool.allocate(1)?.pop().unwrap();
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

        Ok(FrameData {
            context,
            fence,
            commandpool,
            commandbuffer,
            set,
            global_uniformbuffer,
        })
    }
}

impl Drop for FrameData {
    fn drop(&mut self) {
        fence::destroy(self.context.device(), self.fence);
    }
}

struct WindowLayer {
    glfw: Glfw,
    _window: Window,
    events: mpsc::Receiver<(f64, WindowEvent)>,
}

impl WindowLayer {
    pub fn new(glfw: Glfw, window: Window, events: mpsc::Receiver<(f64, WindowEvent)>) -> Self {
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

    fn on_attach(&mut self, _world: &mut World, _events: &mut Events) {}
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
