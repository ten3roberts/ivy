use crate::Result;
use arrayvec::ArrayVec;
use ash::version::DeviceV1_0;
use ash::vk;
use ash::Device;
use std::sync::Arc;

pub use vk::AttachmentDescription;
pub use vk::AttachmentLoadOp as LoadOp;
pub use vk::AttachmentReference;
pub use vk::AttachmentStoreOp as StoreOp;
pub use vk::Format;
pub use vk::ImageLayout;
pub use vk::SubpassDependency;

pub const MAX_ATTACHMENTS: usize = 8;
pub const MAX_SUBPASSES: usize = 8;

#[derive(Debug, Clone, Copy)]
/// Specifies a value to clear each attachment with.
pub enum ClearValue {
    Color(f32, f32, f32, f32),
    DepthStencil(f32, u32),
}

impl From<ClearValue> for vk::ClearValue {
    fn from(val: ClearValue) -> Self {
        match val {
            ClearValue::Color(r, g, b, a) => vk::ClearValue {
                color: vk::ClearColorValue {
                    float32: [r, g, b, a],
                },
            },
            ClearValue::DepthStencil(depth, stencil) => vk::ClearValue {
                depth_stencil: vk::ClearDepthStencilValue { depth, stencil },
            },
        }
    }
}

impl From<&ClearValue> for vk::ClearValue {
    fn from(val: &ClearValue) -> Self {
        (*val).into()
    }
}

#[derive(Debug)]
pub struct SubpassInfo<'a, 'b, 'c> {
    pub color_attachments: &'a [vk::AttachmentReference],
    /// The attachment indices to use as resolve attachmetns
    pub resolve_attachments: &'b [vk::AttachmentReference],
    pub input_attachments: &'c [vk::AttachmentReference],
    pub depth_attachment: Option<AttachmentReference>,
}

impl<'a, 'b, 'c> From<&SubpassInfo<'a, 'b, 'c>> for vk::SubpassDescription {
    fn from(val: &SubpassInfo<'a, 'b, 'c>) -> Self {
        vk::SubpassDescription {
            flags: vk::SubpassDescriptionFlags::default(),
            pipeline_bind_point: vk::PipelineBindPoint::GRAPHICS,
            input_attachment_count: val.input_attachments.len() as u32,
            p_input_attachments: val.input_attachments.as_ptr(),
            color_attachment_count: val.color_attachments.len() as u32,
            p_color_attachments: val.color_attachments.as_ptr(),
            p_resolve_attachments: if !val.resolve_attachments.is_empty() {
                val.resolve_attachments.as_ptr()
            } else {
                std::ptr::null()
            },
            p_depth_stencil_attachment: match &val.depth_attachment {
                Some(attachment) => attachment,
                None => std::ptr::null(),
            },
            preserve_attachment_count: 0,
            p_preserve_attachments: std::ptr::null(),
        }
    }
}

#[derive(Debug)]
/// Specifies renderpass creation info. For array conversion reasons, the number of attachments
/// cannot be more than `MAX_ATTACHMENTS` and subpasses no more than `MAX_SUBPASSES`.
pub struct RenderPassInfo<'a, 'b, 'c, 'd, 'e, 'f> {
    pub attachments: &'a [AttachmentDescription],
    pub subpasses: &'b [SubpassInfo<'c, 'd, 'e>],
    pub dependencies: &'f [SubpassDependency],
}

pub struct RenderPass {
    device: Arc<Device>,
    renderpass: vk::RenderPass,
}

impl RenderPass {
    pub fn new(device: Arc<Device>, info: &RenderPassInfo) -> Result<Self> {
        let vk_subpasses = info
            .subpasses
            .iter()
            .map(|subpass| subpass.into())
            .collect::<ArrayVec<[vk::SubpassDescription; MAX_SUBPASSES]>>();

        let create_info = vk::RenderPassCreateInfo::builder()
            .attachments(&info.attachments)
            .dependencies(info.dependencies)
            .subpasses(&vk_subpasses);

        let renderpass = unsafe { device.create_render_pass(&create_info, None)? };

        Ok(RenderPass { device, renderpass })
    }

    pub fn renderpass(&self) -> vk::RenderPass {
        self.renderpass
    }
}

impl Drop for RenderPass {
    fn drop(&mut self) {
        unsafe { self.device.destroy_render_pass(self.renderpass, None) }
    }
}
