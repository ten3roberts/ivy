use crate::{NodeKind, Result};
use ivy_resources::Handle;
use ivy_vulkan::{
    context::SharedVulkanContext,
    traits::FromExtent,
    vk::{self, ImageBlit, ImageSubresourceLayers, Offset3D, PipelineStageFlags},
    ImageLayout, PassInfo, Texture,
};
use std::slice;

use crate::Node;

pub struct TransferNode {
    src: Handle<Texture>,
    dst: Handle<Texture>,
    // Barrier from renderpass to transfer
    dst_barrier: vk::ImageMemoryBarrier,
    // Barrier from transfer to presentation
    output_barrier: vk::ImageMemoryBarrier,
    initial_layout: ImageLayout,
}

unsafe impl Send for TransferNode {}
unsafe impl Sync for TransferNode {}

impl TransferNode {
    pub fn new(
        context: SharedVulkanContext,
        src: Handle<Texture>,
        dst: Handle<Texture>,
        initial_layout: ImageLayout,
        final_layout: ImageLayout,
    ) -> Result<Self> {
        // Transition transfer image to transfer dst
        let dst_barrier = vk::ImageMemoryBarrier {
            src_access_mask: vk::AccessFlags::default(),
            dst_access_mask: vk::AccessFlags::TRANSFER_READ,
            old_layout: ImageLayout::UNDEFINED,
            new_layout: ImageLayout::TRANSFER_DST_OPTIMAL,
            src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            },
            ..Default::default()
        };

        let output_barrier = vk::ImageMemoryBarrier {
            src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
            dst_access_mask: vk::AccessFlags::MEMORY_READ,
            old_layout: ImageLayout::TRANSFER_DST_OPTIMAL,
            new_layout: final_layout,
            src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: vk::ImageAspectFlags::COLOR,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            },
            ..Default::default()
        };

        Ok(Self {
            dst_barrier,
            output_barrier,
            src,
            dst,
            initial_layout,
        })
    }
}

impl Node for TransferNode {
    fn output_attachments(&self) -> &[Handle<Texture>] {
        slice::from_ref(&self.dst)
    }

    fn read_attachments(&self) -> &[Handle<Texture>] {
        slice::from_ref(&self.src)
    }

    fn node_kind(&self) -> NodeKind {
        NodeKind::Transfer
    }

    fn debug_name(&self) -> &'static str {
        "Transfer Node"
    }

    fn execute(
        &mut self,
        _world: &mut hecs::World,
        resources: &ivy_resources::Resources,
        cmd: &ivy_vulkan::commands::CommandBuffer,
        _: &PassInfo,
        _current_frame: usize,
    ) -> anyhow::Result<()> {
        let dst = resources.get(self.dst)?;

        let src = resources.get(self.src)?;
        let offset = Offset3D::from_extent(src.extent());

        let dst_barrier = vk::ImageMemoryBarrier {
            image: dst.image(),
            ..self.dst_barrier
        };

        cmd.pipeline_barrier(
            PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            PipelineStageFlags::TRANSFER,
            &[],
            &[dst_barrier],
        );

        cmd.blit_image(
            src.image(),
            ImageLayout::TRANSFER_SRC_OPTIMAL,
            dst.image(),
            ImageLayout::TRANSFER_DST_OPTIMAL,
            &[ImageBlit {
                src_subresource: ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                dst_subresource: ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                src_offsets: [Offset3D::default(), offset],
                dst_offsets: [Offset3D::default(), offset],
            }],
            vk::Filter::LINEAR,
        );

        let barrier = vk::ImageMemoryBarrier {
            image: dst.image(),
            ..self.output_barrier
        };

        cmd.pipeline_barrier(
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            &[],
            &[barrier],
        );

        Ok(())
    }
}
