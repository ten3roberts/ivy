use std::ops::Index;

use hecs::World;
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{
    commands::CommandBuffer, descriptors::DescriptorSet, vk::ClearValue, ImageLayout, LoadOp,
    StoreOp, Texture,
};

/// Represents creation info for constructing a node.
/// Dependencies between subpasses will be automatically generated.
/// Each node corresponds to a single renderpass with a subpass.
/// TODO merge adjacent nodes into multiple subpasses.
pub struct NodeInfo {
    // Specifies the color attachments written to by node.
    pub color_attachments: Vec<AttachmentInfo>,
    // Species all the attachment the shaders will use in this pass as descriptors. Dependencies
    // will be generated such that passes with the same `write_attachments` will be executed before and
    // synced.
    pub read_attachments: Vec<AttachmentResource>,
    // An optional depth attachment. Dependencies will be generated the same way as for a
    // `write_attachment`
    pub depth_attachment: Option<AttachmentInfo>,
    pub clear_values: Vec<ClearValue>,
    pub node: Box<dyn Node>,
}

// Trait for the executable functionality of a node
pub trait Node {
    fn execute(
        &mut self,
        world: &mut World,
        commandbuffers: &CommandBuffer,
        current_frame: usize,
        global_set: DescriptorSet,
        resources: &Resources,
    ) -> anyhow::Result<()>;
}

impl<T> Node for T
where
    T: FnMut(
        &mut World,
        &CommandBuffer,
        usize,
        DescriptorSet,
        &Resources,
    ) -> anyhow::Result<()>,
{
    fn execute(
        &mut self,
        world: &mut World,
        commandbuffers: &CommandBuffer,
        current_frame: usize,
        global_set: DescriptorSet,
        resources: &Resources,
    ) -> anyhow::Result<()> {
        (self)(world, commandbuffers, current_frame, global_set, resources)
    }
}

#[derive(Debug, Clone)]
pub struct AttachmentInfo {
    // TODO, derive from edges
    pub store_op: StoreOp,
    pub load_op: LoadOp,
    pub initial_layout: ImageLayout,
    pub final_layout: ImageLayout,
    pub resource: AttachmentResource,
}

impl Default for AttachmentInfo {
    fn default() -> Self {
        Self {
            store_op: StoreOp::STORE,
            load_op: LoadOp::DONT_CARE,
            initial_layout: ImageLayout::UNDEFINED,
            final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            resource: AttachmentResource::Single(Handle::invalid()),
        }
    }
}

impl PartialEq for AttachmentInfo {
    fn eq(&self, other: &Self) -> bool {
        self.resource == other.resource
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum AttachmentResource {
    Single(Handle<Texture>),
    PerFrame(Vec<Handle<Texture>>),
}

impl Index<usize> for AttachmentResource {
    type Output = Handle<Texture>;

    fn index(&self, index: usize) -> &Self::Output {
        match self {
            AttachmentResource::Single(a) => &a,
            AttachmentResource::PerFrame(a) => &a[index],
        }
    }
}

impl From<Vec<Handle<Texture>>> for AttachmentResource {
    fn from(handles: Vec<Handle<Texture>>) -> Self {
        AttachmentResource::PerFrame(handles)
    }
}

impl From<Handle<Texture>> for AttachmentResource {
    fn from(handle: Handle<Texture>) -> Self {
        AttachmentResource::Single(handle)
    }
}
