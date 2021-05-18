use std::sync::Arc;

use super::{renderpass::MAX_ATTACHMENTS, Error, Extent, RenderPass};
use arrayvec::ArrayVec;
use ash::version::DeviceV1_0;
use ash::vk;
use ash::Device;

/// A framebuffer wraps one or more Textures contained in a renderpass.
/// The framebuffer does not own the Textures and as such the user must ensure the referenced
/// textures are kept alive. This is because a texture can be used in several framebuffers
/// simultaneously.
pub struct Framebuffer {
    device: Arc<Device>,
    framebuffer: vk::Framebuffer,
    extent: Extent,
}

impl Framebuffer {
    pub fn new<T: AsRef<vk::ImageView>>(
        device: Arc<Device>,
        renderpass: &RenderPass,
        attachments: &[T],
        extent: Extent,
    ) -> Result<Self, Error> {
        let attachment_views = attachments
            .iter()
            .map(|attachment| *attachment.as_ref())
            .collect::<ArrayVec<[vk::ImageView; MAX_ATTACHMENTS]>>();

        let create_info = vk::FramebufferCreateInfo::builder()
            .render_pass(renderpass.renderpass())
            .attachments(&attachment_views)
            .width(extent.width)
            .height(extent.height)
            .layers(1);

        let framebuffer = unsafe { device.create_framebuffer(&create_info, None)? };

        Ok(Framebuffer {
            device,
            framebuffer,
            extent,
        })
    }

    pub fn framebuffer(&self) -> vk::Framebuffer {
        self.framebuffer
    }

    pub fn extent(&self) -> Extent {
        self.extent
    }
}

impl Drop for Framebuffer {
    fn drop(&mut self) {
        unsafe { self.device.destroy_framebuffer(self.framebuffer, None) }
    }
}
