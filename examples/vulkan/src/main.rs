use std::{rc::Rc, time::Duration};

use glfw::*;
use ivy_core::*;
use ivy_graphics::window::{WindowExt, WindowInfo, WindowMode};
use ivy_vulkan::commands::*;
use ivy_vulkan::descriptors::*;
use ivy_vulkan::*;
use ultraviolet::{projection, Mat4, Vec2, Vec3, Vec4};

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
            extent: None,
            resizable: false,
            mode: WindowMode::Windowed,
        },
    )?;

    let extent = window.extent();

    // Initialize vulkan
    let context = Rc::new(VulkanContext::new(&glfw, &window)?);

    // Create the link between vulkan and the window presentation
    let swapchain = Swapchain::new(context.clone(), &window)?;

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

    let mut descriptor_layout_cache = DescriptorLayoutCache::new(context.device_ref());

    let mut descriptor_allocator = DescriptorAllocator::new(context.device_ref(), 2);

    // Synchronization primitives for the draw loop
    let image_available_semaphores = (0..FRAMES_IN_FLIGHT)
        .into_iter()
        .map(|_| semaphore::create(context.device()))
        .collect::<Result<Vec<_>, _>>()?;

    let render_finished_semaphores = (0..FRAMES_IN_FLIGHT)
        .into_iter()
        .map(|_| semaphore::create(context.device()))
        .collect::<Result<Vec<_>, _>>()?;

    let in_flight_fences = (0..FRAMES_IN_FLIGHT)
        .into_iter()
        .map(|_| fence::create(context.device(), true))
        .collect::<Result<Vec<_>, _>>()?;

    // Data that is tied and updated per swapchain image basis
    let mut frames = swapchain
        .images()
        .iter()
        .map(|swapchain_image| {
            PerFrameData::new(
                context.clone(),
                &renderpass,
                &depth_attachment,
                swapchain_image,
                &mut descriptor_allocator,
                &mut descriptor_layout_cache,
            )
        })
        .collect::<Result<Vec<PerFrameData>, _>>()?;

    let document = ivy_graphics::Document::load(context.clone(), "./res/models/cube.gltf")?;

    let mesh = document.mesh(0).clone();

    let viewproj = ultraviolet::projection::perspective_vk(1.0, extent.aspect(), 0.1, 100.0)
        * ultraviolet::Mat4::look_at(Vec3::new(5.0, 0.5, 5.0), Vec3::zero(), Vec3::unit_y());

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

    info!("Entering event loop");
    let mut frame_in_flight = 0;

    let mut frame_clock = Clock::new();
    let mut status_clock = Clock::new();

    while !window.should_close() {
        glfw.poll_events();
        for (_, event) in glfw::flush_messages(&events) {
            debug!("Event: {:?}", event);
        }
        let dt = frame_clock.reset();
        if status_clock.elapsed() > Duration::from_millis(500) {
            status_clock.reset();
            info!("Frametime: {:#?},\t Framerate: {:#?}", dt, 1.0 / dt.secs());
        }

        // Wait for current frame in flight to not be in use
        fence::wait(context.device(), &[in_flight_fences[frame_in_flight]], true)?;

        // Acquire the next image from swapchain
        let image_index = match swapchain.next_image(image_available_semaphores[frame_in_flight]) {
            Ok(image_index) => image_index,
            Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                return Ok(());
            }

            Err(e) => return Err(e.into()),
        };

        // Extract data for this image in swapchain
        let frame = &mut frames[image_index as usize];

        // Wait if previous frame is using this image
        if frame.image_in_flight != Fence::null() {
            fence::wait(context.device(), &[frame.image_in_flight], true)?;
        }

        // Mark the image as being used by the frame in flight
        frame.image_in_flight = in_flight_fences[frame_in_flight];

        frame.commandpool.reset(false)?;

        // Update global uniform buffer
        frame.global_uniformbuffer.fill(0, &[global_data])?;

        // Begin the commandbuffer, hinting that it will only be used once
        frame
            .commandbuffer
            .begin(vk::CommandBufferUsageFlags::ONE_TIME_SUBMIT)?;

        frame.commandbuffer.begin_renderpass(
            &renderpass,
            &frame.framebuffer,
            swapchain.extent(),
            &[
                ClearValue::Color(0.0, 0.0, 0.0, 0.0),
                ClearValue::DepthStencil(1.0, 0),
            ],
        );

        // Bind the global uniform buffer
        frame
            .commandbuffer
            .bind_descriptor_sets(pipeline.layout(), 0, &[frame.set]);

        // Bind the pipeline
        frame.commandbuffer.bind_pipeline(&pipeline);

        // Bind and draw the triangle without indexing
        frame
            .commandbuffer
            .bind_vertexbuffers(0, &[&mesh.vertex_buffer()]);
        frame.commandbuffer.bind_indexbuffer(mesh.index_buffer(), 0);

        frame
            .commandbuffer
            .draw_indexed(mesh.index_count(), 1, 0, 0, 0);

        // Done
        frame.commandbuffer.end_renderpass();
        frame.commandbuffer.end()?;

        // Which synchronization primities to wait on
        let wait_semaphores = [image_available_semaphores[frame_in_flight]];

        let signal_semaphores = [render_finished_semaphores[frame_in_flight]];

        // Reset fence before
        fence::reset(context.device(), &[in_flight_fences[frame_in_flight]])?;

        // Submit command buffers
        frame.commandbuffer.submit(
            context.graphics_queue(),
            &wait_semaphores,
            &signal_semaphores,
            in_flight_fences[frame_in_flight],
            &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT],
        )?;

        let _suboptimal =
            match swapchain.present(context.present_queue(), &signal_semaphores, image_index) {
                Ok(image_index) => image_index,
                Err(vk::Result::ERROR_OUT_OF_DATE_KHR) => {
                    return Ok(());
                }

                Err(e) => return Err(e.into()),
            };

        frame_in_flight = (frame_in_flight + 1) % FRAMES_IN_FLIGHT as usize;
    }

    // Wait for everything to be done before cleaning up
    device::wait_idle(context.device()).unwrap();

    image_available_semaphores
        .iter()
        .for_each(|s| semaphore::destroy(context.device(), *s));

    render_finished_semaphores
        .iter()
        .for_each(|s| semaphore::destroy(context.device(), *s));

    in_flight_fences
        .iter()
        .for_each(|f| fence::destroy(context.device(), *f));

    Ok(())
}

/// Represents data needed to be duplicated for each swapchain image
struct PerFrameData {
    commandpool: CommandPool,
    commandbuffer: CommandBuffer,
    framebuffer: Framebuffer,
    // The fence currently associated to this image_index
    image_in_flight: Fence,
    set: DescriptorSet,
    global_uniformbuffer: Buffer,
}

impl PerFrameData {
    fn new(
        context: Rc<VulkanContext>,
        renderpass: &RenderPass,
        depth_attachment: &Texture,
        swapchain_image: &Texture,
        descriptor_allocator: &mut DescriptorAllocator,
        descriptor_layout_cache: &mut DescriptorLayoutCache,
    ) -> Result<Self, ivy_vulkan::Error> {
        let framebuffer = Framebuffer::new(
            context.device_ref(),
            &renderpass,
            &[swapchain_image, depth_attachment],
            swapchain_image.extent(),
        )?;

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

        Ok(PerFrameData {
            framebuffer,
            commandpool,
            commandbuffer,
            image_in_flight: Fence::null(),
            global_uniformbuffer,
            set,
        })
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
