use hecs::World;
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{commands::CommandBuffer, vk::ClearValue, ImageLayout, LoadOp, StoreOp, Texture};

/// Represents a node in the renderpass.
pub trait Node {
    /// Returns the color attachments for this node. Should not be execution heavy function
    fn color_attachments(&self) -> &[AttachmentInfo];
    /// Returns the read attachments for this node. Should not be execution heavy function
    fn read_attachments(&self) -> &[Handle<Texture>];
    /// Partially samples input attachments. Read from the same pixel coord we write to
    fn input_attachments(&self) -> &[Handle<Texture>];
    /// Returns the optional depth attachment for this node. Should not be execution heavy function
    fn depth_attachment(&self) -> Option<&AttachmentInfo>;

    /// Returns the clear values to initiate this renderpass
    fn clear_values(&self) -> &[ClearValue];

    fn node_kind(&self) -> NodeKind;

    // Optional name, can be empty string
    fn debug_name(&self) -> &str {
        "Unnamed node"
    }

    /// Execute this node inside a compatible renderpass
    fn execute(
        &mut self,
        world: &mut World,
        cmd: &CommandBuffer,
        current_frame: usize,
        resources: &Resources,
    ) -> anyhow::Result<()>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    // A graphics rendering node. Renderpass and framebuffer will automatically be created.
    Graphics,
    // execution
    // A node that will be executed on the transfer queue. Appropriate pipeline barriers will
    // be inserted
    Transfer,
    // Compute,
}

#[derive(Debug, Clone)]
pub struct AttachmentInfo {
    // TODO, derive from edges
    pub store_op: StoreOp,
    pub load_op: LoadOp,
    pub initial_layout: ImageLayout,
    pub final_layout: ImageLayout,
    pub resource: Handle<Texture>,
}

impl Default for AttachmentInfo {
    fn default() -> Self {
        Self {
            store_op: StoreOp::STORE,
            load_op: LoadOp::DONT_CARE,
            initial_layout: ImageLayout::UNDEFINED,
            final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            resource: Handle::null(),
        }
    }
}
