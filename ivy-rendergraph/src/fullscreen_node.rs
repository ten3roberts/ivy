use anyhow::Context;
use ivy_graphics::{Renderer, ShaderPass};
use ivy_resources::{Handle, Storage};
use ivy_vulkan::{
    descriptors::{DescriptorSet, IntoSet},
    vk::ClearValue,
    Texture,
};
use std::marker::PhantomData;

use crate::{AttachmentInfo, Node, NodeKind};

pub struct FullscreenNode<Pass, T, E> {
    renderer: Handle<T>,
    marker: PhantomData<(Pass, E)>,
    color_attachments: Vec<AttachmentInfo>,
    read_attachments: Vec<Handle<Texture>>,
    input_attachments: Vec<Handle<Texture>>,
    depth_attachment: Option<AttachmentInfo>,
    clear_values: Vec<ClearValue>,
    sets: Vec<DescriptorSet>,
    set_count: usize,
}

impl<Pass, T, E> FullscreenNode<Pass, T, E>
where
    Pass: ShaderPass + Storage,
    T: Renderer<Error = E> + Storage,
    E: Into<anyhow::Error>,
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
    ) -> Self {
        let set_count = sets.len();

        let set_iter = sets.iter().map(|val| val.sets().into_iter().cycle());

        let sets = (0..frames_in_flight)
            .flat_map(|_| set_iter.clone().map(|mut val| (*val.next().unwrap())))
            .collect::<Vec<_>>();

        Self {
            renderer,
            marker: PhantomData,
            color_attachments,
            read_attachments,
            input_attachments,
            depth_attachment,
            clear_values,
            sets,
            set_count,
        }
    }
}

impl<Pass, T, E> Node for FullscreenNode<Pass, T, E>
where
    Pass: ShaderPass + Storage,
    T: Renderer<Error = E> + Storage,
    E: 'static + std::error::Error + Sync + Send,
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
        world: &mut hecs::World,
        cmd: &ivy_vulkan::commands::CommandBuffer,
        current_frame: usize,
        resources: &ivy_resources::Resources,
    ) -> anyhow::Result<()> {
        let offset = self.set_count * current_frame;
        resources
            .get_mut(self.renderer)?
            .draw::<Pass>(
                world,
                cmd,
                current_frame,
                &self.sets[offset..offset + self.set_count],
                &[],
                resources,
            )
            .context("FullscreenNode failed to draw using supplied renderer")?;

        Ok(())
    }
}
