use anyhow::Context;
use flax::{Component, World};
use ivy_assets::{Asset, AssetCache};
use ivy_graphics::Renderer;
use ivy_vulkan::{
    descriptors::{DescriptorSet, IntoSet},
    vk::ClearValue,
    PassInfo, Shader, Texture,
};

use crate::{AttachmentInfo, Node, NodeKind};

pub struct FullscreenNode<T> {
    renderer: T,
    color_attachments: Vec<AttachmentInfo>,
    read_attachments: Vec<Asset<Texture>>,
    input_attachments: Vec<Asset<Texture>>,
    depth_attachment: Option<AttachmentInfo>,
    clear_values: Vec<ClearValue>,
    sets: Vec<DescriptorSet>,
    set_count: usize,
    shaderpass: Component<Shader>,
}

impl<T> FullscreenNode<T>
where
    T: Renderer,
{
    pub fn new(
        renderer: T,
        color_attachments: Vec<AttachmentInfo>,
        read_attachments: Vec<Asset<Texture>>,
        input_attachments: Vec<Asset<Texture>>,
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
    T: 'static + Send + Sync + Renderer,
{
    fn color_attachments(&self) -> &[AttachmentInfo] {
        &self.color_attachments
    }

    fn read_attachments(&self) -> &[Asset<Texture>] {
        &self.read_attachments
    }

    fn input_attachments(&self) -> &[Asset<Texture>] {
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
        assets: &AssetCache,
        cmd: &ivy_vulkan::commands::CommandBuffer,
        pass_info: &PassInfo,
        current_frame: usize,
    ) -> anyhow::Result<()> {
        let offset = self.set_count * current_frame;
        self.renderer
            .draw(
                world,
                assets,
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
