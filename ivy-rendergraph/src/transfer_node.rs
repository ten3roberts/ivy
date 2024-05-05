use crate::{NodeKind, Result};
use flax::World;
use ivy_assets::{Asset, AssetCache};
use ivy_vulkan::{
    traits::FromExtent,
    vk::{
        self, AttachmentSampleCountInfoAMD, ImageAspectFlags, ImageBlit, ImageSubresourceLayers,
        Offset3D, PipelineStageFlags,
    },
    ImageLayout, PassInfo, Texture,
};
use std::slice;

use crate::Node;

pub struct TransferNode {
    src: Asset<Texture>,
    dst: Asset<Texture>,
    // Barrier from renderpass to transfer
    dst_barrier: vk::ImageMemoryBarrier,
    src_barriers: [vk::ImageMemoryBarrier; 2],
    // Barrier from transfer to presentation
    output_barrier: vk::ImageMemoryBarrier,
    aspect: ImageAspectFlags,
}

unsafe impl Send for TransferNode {}
unsafe impl Sync for TransferNode {}

impl TransferNode {
    pub fn new(
        src: Asset<Texture>,
        dst: Asset<Texture>,
        initial_layout: ImageLayout,
        src_final_layout: ImageLayout,
        dst_final_layout: ImageLayout,
        aspect: ImageAspectFlags,
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
                aspect_mask: aspect,
                base_mip_level: 0,
                level_count: 1,
                base_array_layer: 0,
                layer_count: 1,
            },
            ..Default::default()
        };

        // Transition transfer image to transfer dst
        let src_barriers = [
            vk::ImageMemoryBarrier {
                src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                dst_access_mask: vk::AccessFlags::TRANSFER_READ,
                old_layout: initial_layout,
                new_layout: ImageLayout::TRANSFER_SRC_OPTIMAL,
                src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                subresource_range: vk::ImageSubresourceRange {
                    aspect_mask: aspect,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                ..Default::default()
            },
            vk::ImageMemoryBarrier {
                src_access_mask: vk::AccessFlags::TRANSFER_READ,
                dst_access_mask: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_READ,
                old_layout: ImageLayout::TRANSFER_SRC_OPTIMAL,
                new_layout: src_final_layout,
                src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                subresource_range: vk::ImageSubresourceRange {
                    aspect_mask: aspect,
                    base_mip_level: 0,
                    level_count: 1,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                ..Default::default()
            },
        ];
        let output_barrier = vk::ImageMemoryBarrier {
            src_access_mask: vk::AccessFlags::TRANSFER_WRITE,
            dst_access_mask: vk::AccessFlags::MEMORY_READ,
            old_layout: ImageLayout::TRANSFER_DST_OPTIMAL,
            new_layout: dst_final_layout,
            src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
            subresource_range: vk::ImageSubresourceRange {
                aspect_mask: aspect,
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
            aspect,
            src_barriers,
        })
    }
}

impl Node for TransferNode {
    fn output_attachments(&self) -> &[Asset<Texture>] {
        slice::from_ref(&self.dst)
    }

    fn read_attachments(&self) -> &[Asset<Texture>] {
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
        _world: &mut World,
        assets: &AssetCache,
        cmd: &ivy_vulkan::commands::CommandBuffer,
        _: &PassInfo,
        _current_frame: usize,
    ) -> anyhow::Result<()> {
        let dst = self.dst.clone();

        let src = self.src.clone();
        let offset = Offset3D::from_extent(src.extent());

        let src_barrier = vk::ImageMemoryBarrier {
            image: src.image(),
            ..self.src_barriers[0]
        };
        let dst_barrier = vk::ImageMemoryBarrier {
            image: dst.image(),
            ..self.dst_barrier
        };

        cmd.pipeline_barrier(
            PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            PipelineStageFlags::TRANSFER,
            &[],
            &[dst_barrier, src_barrier],
        );

        cmd.blit_image(
            src.image(),
            ImageLayout::TRANSFER_SRC_OPTIMAL,
            dst.image(),
            ImageLayout::TRANSFER_DST_OPTIMAL,
            &[ImageBlit {
                src_subresource: ImageSubresourceLayers {
                    aspect_mask: self.aspect,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                dst_subresource: ImageSubresourceLayers {
                    aspect_mask: self.aspect,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                src_offsets: [Offset3D::default(), offset],
                dst_offsets: [Offset3D::default(), offset],
            }],
            vk::Filter::NEAREST,
        );

        let barrier = vk::ImageMemoryBarrier {
            image: dst.image(),
            ..self.output_barrier
        };
        let src_barrier = vk::ImageMemoryBarrier {
            image: src.image(),
            ..self.src_barriers[1]
        };

        cmd.pipeline_barrier(
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::EARLY_FRAGMENT_TESTS,
            &[],
            &[barrier, src_barrier],
        );

        Ok(())
    }
}
