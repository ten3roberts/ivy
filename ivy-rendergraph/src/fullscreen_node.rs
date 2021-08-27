use std::marker::PhantomData;

use anyhow::Context;
use ivy_graphics::{Renderer, ShaderPass};
use ivy_resources::{Handle, Storage};
use ivy_vulkan::{descriptors::DescriptorSet, vk::ClearValue, Texture};

use crate::{AttachmentInfo, Node, NodeKind};

pub struct FullscreenNode<Pass, T> {
    renderer: Handle<T>,
    marker: PhantomData<Pass>,
    color_attachments: Vec<AttachmentInfo>,
    read_attachments: Vec<Handle<Texture>>,
    input_attachments: Vec<Handle<Texture>>,
    depth_attachment: Option<AttachmentInfo>,
    clear_values: Vec<ClearValue>,
    sets: Vec<DescriptorSet>,
}

impl<Pass, T> FullscreenNode<Pass, T>
where
    Pass: ShaderPass + Storage,
    T: Renderer + Storage,
{
    pub fn new(
        renderer: Handle<T>,
        color_attachments: Vec<AttachmentInfo>,
        read_attachments: Vec<Handle<Texture>>,
        input_attachments: Vec<Handle<Texture>>,
        depth_attachment: Option<AttachmentInfo>,
        clear_values: Vec<ClearValue>,
        sets: Vec<DescriptorSet>,
    ) -> Self {
        Self {
            renderer,
            marker: PhantomData,
            color_attachments,
            read_attachments,
            input_attachments,
            depth_attachment,
            clear_values,
            sets,
        }
    }
}

impl<Pass, T> Node for FullscreenNode<Pass, T>
where
    Pass: ShaderPass + Storage,
    T: Renderer + Storage,
{
    fn color_attachments(&self) -> &[AttachmentInfo] {
        &self.color_attachments
    }

    fn read_attachments(&self) -> &[Handle<Texture>] {
        &self.read_attachments
    }

    fn input_attachments(&self) -> &[Handle<Texture>] {
        &self.input_attachments
    }

    fn depth_attachment(&self) -> Option<&AttachmentInfo> {
        self.depth_attachment.as_ref()
    }

    fn clear_values(&self) -> &[ivy_vulkan::vk::ClearValue] {
        &self.clear_values
    }

    fn node_kind(&self) -> crate::NodeKind {
        NodeKind::Graphics
    }

    fn debug_name(&self) -> &str {
        "fullscreen node"
    }

    fn execute(
        &mut self,
        world: &mut hecs::World,
        cmd: &ivy_vulkan::commands::CommandBuffer,
        current_frame: usize,
        resources: &ivy_resources::Resources,
    ) -> anyhow::Result<()> {
        resources
            .get_mut(self.renderer)?
            .draw::<Pass>(world, cmd, current_frame, &self.sets, &[], resources)
            .context("FullscreenNode failed to draw using supplied renderer")?;

        Ok(())
    }
}
