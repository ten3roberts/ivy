use crate::{NodeKind, Result};
use anyhow::Context;
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{
    context::SharedVulkanContext,
    traits::FromExtent,
    vk::{self, ImageBlit, ImageSubresourceLayers, Offset3D, PipelineStageFlags},
    ImageLayout, ImageUsage, PassInfo, SampleCountFlags, Swapchain, Texture, TextureInfo,
};
use std::{ops::Deref, slice};

use crate::{AttachmentInfo, Node};

pub struct SwapchainNode {
    swapchain: Handle<Swapchain>,
    read_attachment: Handle<Texture>,
    swapchain_images: Vec<Handle<Texture>>,
    // Barrier from renderpass to transfer
    dst_barrier: vk::ImageMemoryBarrier,
    // Barrier from transfer to presentation
    output_barrier: vk::ImageMemoryBarrier,
}

unsafe impl Send for SwapchainNode {}
unsafe impl Sync for SwapchainNode {}

impl SwapchainNode {
    pub fn new(
        context: SharedVulkanContext,
        resources: &Resources,
        swapchain: Handle<Swapchain>,
        read_attachment: Handle<Texture>,
    ) -> Result<Self> {
        let swapchain_ref = resources.get(swapchain)?;

        let texture_info = TextureInfo {
            extent: swapchain_ref.extent(),
            mip_levels: 1,
            usage: ImageUsage::TRANSFER_DST,
            format: swapchain_ref.image_format(),
            samples: SampleCountFlags::TYPE_1,
        };

        let swapchain_images = swapchain_ref
            .images()
            .iter()
            .map(|image| -> Result<_> {
                Texture::from_image(context.clone(), &texture_info, *image, None, 1, 0)
                    .map_err(|e| e.into())
                    .and_then(|val| resources.insert(val).map_err(|e| e.into()))
            })
            .collect::<Result<Vec<_>>>()?;

        // Transition swapchain image to transfer dst
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
            new_layout: ImageLayout::PRESENT_SRC_KHR,
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
            swapchain,
            read_attachment,
            swapchain_images,
            dst_barrier,
            output_barrier,
        })
    }
}

impl Node for SwapchainNode {
    fn color_attachments(&self) -> &[AttachmentInfo] {
        &[]
    }

    fn read_attachments(&self) -> &[Handle<Texture>] {
        slice::from_ref(&self.read_attachment)
    }

    fn input_attachments(&self) -> &[Handle<Texture>] {
        &[]
    }

    fn depth_attachment(&self) -> Option<&AttachmentInfo> {
        None
    }

    fn clear_values(&self) -> &[ivy_vulkan::vk::ClearValue] {
        &[]
    }

    fn node_kind(&self) -> NodeKind {
        NodeKind::Transfer
    }

    fn debug_name(&self) -> &'static str {
        "Swapchain Node"
    }

    fn execute(
        &mut self,
        _world: &mut hecs::World,
        resources: &ivy_resources::Resources,
        cmd: &ivy_vulkan::commands::CommandBuffer,
        _: &PassInfo,
        _current_frame: usize,
    ) -> anyhow::Result<()> {
        let swapchain = resources.get(self.swapchain)?;
        let extent = swapchain.extent();
        let offset = Offset3D::from_extent(extent);

        let image_index = swapchain
            .image_index()
            .context("Failed to get image index from swapchain")?;

        let dst = resources.get(self.swapchain_images[image_index as usize])?;

        let src = resources.get(self.read_attachment)?;

        let dst_barrier = vk::ImageMemoryBarrier {
            image: dst.deref().image(),
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
