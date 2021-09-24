use anyhow::Context;
use ivy_resources::Handle;
use ivy_vulkan::Texture;

use crate::Node;

/// A rendergraph node holding two other nodes.
/// The two nodes *must* have the same attachments.
pub struct DoubleNode<A, B> {
    a: A,
    b: B,
    read_attachments: Vec<Handle<Texture>>,
}

impl<A: Node, B: Node> DoubleNode<A, B> {
    pub fn new(a: A, b: B) -> Self {
        let read_attachments = a
            .read_attachments()
            .iter()
            .cloned()
            .chain(b.read_attachments().iter().cloned())
            .collect::<Vec<_>>();

        Self {
            a,
            b,
            read_attachments,
        }
    }
}

impl<A: Node, B: Node> Node for DoubleNode<A, B> {
    fn node_kind(&self) -> crate::NodeKind {
        crate::NodeKind::Graphics
    }

    fn execute(
        &mut self,
        world: &mut hecs::World,
        cmd: &ivy_vulkan::commands::CommandBuffer,
        current_frame: usize,
        resources: &ivy_resources::Resources,
    ) -> anyhow::Result<()> {
        self.a
            .execute(world, cmd, current_frame, resources)
            .with_context(|| {
                format!(
                    "Failed to execute first node {:?} in double node",
                    self.a.debug_name()
                )
            })?;

        self.b
            .execute(world, cmd, current_frame, resources)
            .with_context(|| {
                format!(
                    "Failed to execute second node {:?} in double node",
                    self.b.debug_name()
                )
            })?;

        Ok(())
    }

    fn color_attachments(&self) -> &[crate::AttachmentInfo] {
        &self.a.color_attachments()
    }

    fn read_attachments(&self) -> &[ivy_resources::Handle<ivy_vulkan::Texture>] {
        &self.read_attachments
    }

    fn input_attachments(&self) -> &[ivy_resources::Handle<ivy_vulkan::Texture>] {
        self.a.input_attachments()
    }

    fn depth_attachment(&self) -> Option<&crate::AttachmentInfo> {
        self.a.depth_attachment()
    }

    fn clear_values(&self) -> &[ivy_vulkan::vk::ClearValue] {
        self.a.clear_values()
    }

    fn debug_name(&self) -> &'static str {
        "Double node"
    }
}
