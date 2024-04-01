use anyhow::Context;
use flax::{Component, World};
use ivy_graphics::Renderer;
use ivy_resources::{Handle, Storage};
use ivy_vulkan::{
    descriptors::{DescriptorSet, IntoSet},
    vk::ClearValue,
    PassInfo, Shader, Texture,
};

use crate::{AttachmentInfo, Node, NodeKind};

pub struct FullscreenNode<T> {
    renderer: Handle<T>,
    color_attachments: Vec<AttachmentInfo>,
    read_attachments: Vec<Handle<Texture>>,
    input_attachments: Vec<Handle<Texture>>,
    depth_attachment: Option<AttachmentInfo>,
    clear_values: Vec<ClearValue>,
    sets: Vec<DescriptorSet>,
    set_count: usize,
    shaderpass: Component<Shader>,
}

impl<T> FullscreenNode<T>
where
    T: Renderer + Storage,
{
    pub fn new(
        renderer: Handle<T>,
        color_attachments: Vec<AttachmentInfo>,
        read_attachments: Vec<Handle<Texture>>,
        input_attachments: Vec<Handle<Texture>>,
        depth_attachment: Option<AttachmentInfo>,
        clear_values: Vec<ClearValue>,
        sets: Vec<&dyn IntoSet>,
        frames_in_flight: usize,
        shaderpass: Component<Shader>,
    ) -> Self {
        let set_count = sets.len();

        let set_iter = sets.iter().map(|val| val.sets().into_iter().cycle());

        let sets = (0..frames_in_flight)
            .flat_map(|_| set_iter.clone().map(|mut val| (*val.next().unwrap())))
            .collect::<Vec<_>>();

        Self {
            renderer,
            color_attachments,
            read_attachments,
            input_attachments,
            depth_attachment,
            clear_values,
            sets,
            set_count,
            shaderpass,
        }
    }
}

impl<T> Node for FullscreenNode<T>
where
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

    fn debug_name(&self) -> &'static str {
        "fullscreen node"
    }

    fn execute(
        &mut self,
        world: &mut World,
        resources: &ivy_resources::Resources,
        cmd: &ivy_vulkan::commands::CommandBuffer,
        pass_info: &PassInfo,
        current_frame: usize,
    ) -> anyhow::Result<()> {
        let offset = self.set_count * current_frame;
        resources
            .get_mut(self.renderer)?
            .draw(
                world,
                resources,
                cmd,
                &self.sets[offset..offset + self.set_count],
                pass_info,
                &[],
                current_frame,
                self.shaderpass,
            )
            .context("FullscreenNode failed to draw using supplied renderer")?;

        Ok(())
    }
}
