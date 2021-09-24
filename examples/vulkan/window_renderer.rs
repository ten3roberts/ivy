use crate::Result;
use atomic_refcell::AtomicRefCell;
use std::sync::Arc;

use glfw::Window;
use ivy_vulkan::{
    commands::CommandBuffer,
    device, semaphore,
    vk::{self, Semaphore},
    AttachmentInfo, AttachmentReference, ClearValue, Fence, Format, Framebuffer, ImageLayout,
    ImageUsage, LoadOp, RenderPass, RenderPassInfo, SampleCountFlags, StoreOp, SubpassInfo,
    Swapchain, SwapchainInfo, Texture, TextureInfo, VulkanContext,
};

/// Renderer rendering to a glfw window
pub struct WindowRenderer {
    context: Arc<VulkanContext>,
    swapchain: Swapchain,
    _window: Arc<AtomicRefCell<Window>>,

    framebuffers: Vec<Framebuffer>,
    _depth_attachment: Texture,

    render_semaphore: Semaphore,
    present_semaphore: Semaphore,

    renderpass: RenderPass,

    image_index: u32,
}

impl WindowRenderer {
    pub fn new(
        context: Arc<VulkanContext>,
        window: Arc<AtomicRefCell<Window>>,
        swapchain_info: SwapchainInfo,
    ) -> Result<Self> {
        let swapchain = Swapchain::new(context.clone(), &window.borrow(), swapchain_info)?;

        let extent = swapchain.extent();

        // Depth buffer attachment
        let depth_attachment = Texture::new(
            context.clone(),
            &TextureInfo {
                extent: swapchain.extent(),
                mip_levels: 1,
                usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT,
                format: Format::D32_SFLOAT,
                samples: SampleCountFlags::TYPE_1,
            },
        )?;

        // Create a simple renderpass rendering directly to the swapchain images
        let renderpass_info = RenderPassInfo {
            attachments: &[
                AttachmentInfo::from_texture(
                    swapchain.images()[0],
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
                input_attachments: &[],
                depth_attachment: Some(AttachmentReference {
                    attachment: 1,
                    layout: ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                }),
            }],
        };

        let renderpass = RenderPass::new(context.device().clone(), &renderpass_info)?;

        let framebuffers = swapchain
            .images()
            .iter()
            .map(|image| {
                Framebuffer::new(
                    context.device().clone(),
                    &renderpass,
                    &[image, &depth_attachment],
                    extent,
                )
                .map_err(|e| e.into())
            })
            .collect::<Result<Vec<_>>>()?;

        let render_semaphore = semaphore::create(context.device())?;
        let present_semaphore = semaphore::create(context.device())?;

        Ok(Self {
            context,
            swapchain,
            _window: window,
            framebuffers,
            _depth_attachment: depth_attachment,
            render_semaphore,
            present_semaphore,
            renderpass,
            image_index: 0,
        })
    }

    /// Acquires the next swapchain image and starts a renderpass in the passed commandbuffer
    pub fn begin(&mut self, commandbuffer: &CommandBuffer) -> Result<()> {
        self.image_index = self.swapchain.next_image(self.present_semaphore).unwrap();

        let framebuffer = &self.framebuffers[self.image_index as usize];

        commandbuffer.begin_renderpass(
            &self.renderpass,
            &framebuffer,
            self.swapchain.extent(),
            &[
                ClearValue::Color(0.0, 0.0, 0.0, 1.0),
                ClearValue::DepthStencil(1.0, 0),
            ],
        );

        Ok(())
    }

    /// Submits and presents the commandbuffer results to the window.
    /// Signals the provided fence when done.
    pub fn submit(&mut self, commandbuffer: &CommandBuffer, fence: Fence) -> Result<()> {
        // // Submit command buffers and signal fence `current_frame` when done
        commandbuffer
            .submit(
                self.context.graphics_queue(),
                &[self.present_semaphore],
                &[self.render_semaphore],
                fence,
                &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT],
            )
            .unwrap();

        self.swapchain.present(
            self.context.present_queue(),
            &[self.render_semaphore],
            self.image_index,
        )?;

        Ok(())
    }

    /// Get a reference to the window renderer's swapchain.
    pub fn swapchain(&self) -> &Swapchain {
        &self.swapchain
    }

    /// Get a reference to the window renderer's renderpass.
    pub fn renderpass(&self) -> &RenderPass {
        &self.renderpass
    }
}

impl Drop for WindowRenderer {
    fn drop(&mut self) {
        let device = self.context.device();
        // Wait for everything to be done before cleaning up
        device::wait_idle(device).unwrap();

        semaphore::destroy(device, self.present_semaphore);
        semaphore::destroy(device, self.render_semaphore);
    }
}
