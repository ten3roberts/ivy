use crate::{NodeKind, Result};
use anyhow::Context;
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{
    vk::{self, ClearValue, ImageCopy, ImageSubresourceLayers, PipelineStageFlags},
    ImageLayout, ImageUsage, SampleCountFlags, Swapchain, Texture, TextureInfo,
};

use crate::{AttachmentInfo, Node};

pub struct SwapchainNode {
    swapchain: Handle<Swapchain>,
    read_attachments: [Handle<Texture>; 1],
    clear_values: Vec<ClearValue>,
    swapchain_images: Vec<Handle<Texture>>,
    // Barrier from renderpass to transfer
    dst_barrier: vk::ImageMemoryBarrier,
    // Barrier from transfer to presentation
    output_barrier: vk::ImageMemoryBarrier,
}

impl SwapchainNode {
    pub fn new(
        swapchain: Handle<Swapchain>,
        read_attachment: Handle<Texture>,
        clear_values: Vec<ClearValue>,
        resources: &Resources,
    ) -> Result<Self> {
        let swapchain_ref = resources.get(swapchain)?;

        let texture_info = TextureInfo {
            extent: swapchain_ref.extent(),
            mip_levels: 1,
            usage: ImageUsage::COLOR_ATTACHMENT,
            format: swapchain_ref.image_format(),
            samples: SampleCountFlags::TYPE_1,
        };

        let context = swapchain_ref.context();

        let swapchain_images = swapchain_ref
            .images()
            .iter()
            .map(|image| -> Result<_> {
                Texture::from_image(context.clone(), &texture_info, *image, None)
                    .map_err(|e| e.into())
                    .and_then(|val| resources.insert(val).map_err(|e| e.into()))
            })
            .collect::<Result<Vec<_>>>()?;

        // // Transition read attachment to transfer src
        // let src_barrier = vk::ImageMemoryBarrier {
        //     src_access_mask: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
        //     dst_access_mask: vk::AccessFlags::TRANSFER_READ,
        //     old_layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
        //     new_layout: ImageLayout::TRANSFER_SRC_OPTIMAL,
        //     src_queue_family_index: context.queue_families().transfer().unwrap(),
        //     dst_queue_family_index: context.queue_families().transfer().unwrap(),
        //     subresource_range: vk::ImageSubresourceRange {
        //         aspect_mask: vk::ImageAspectFlags::COLOR,
        //         base_mip_level: 0,
        //         level_count: 1,
        //         base_array_layer: 0,
        //         layer_count: 1,
        //     },
        //     ..Default::default()
        // };

        // Transition swapchain image to transfer dst
        let dst_barrier = vk::ImageMemoryBarrier {
            src_access_mask: vk::AccessFlags::default(),
            dst_access_mask: vk::AccessFlags::TRANSFER_READ,
            old_layout: ImageLayout::UNDEFINED,
            new_layout: ImageLayout::TRANSFER_DST_OPTIMAL,
            src_queue_family_index: context.queue_families().transfer().unwrap(),
            dst_queue_family_index: context.queue_families().transfer().unwrap(),
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
            src_queue_family_index: context.queue_families().transfer().unwrap(),
            dst_queue_family_index: context.queue_families().transfer().unwrap(),
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
            read_attachments: [read_attachment],
            clear_values,
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
        &self.read_attachments
    }

    fn depth_attachment(&self) -> Option<&AttachmentInfo> {
        None
    }

    fn clear_values(&self) -> &[ivy_vulkan::vk::ClearValue] {
        &self.clear_values
    }

    fn node_kind(&self) -> NodeKind {
        NodeKind::Transfer
    }

    fn execute(
        &mut self,
        _world: &mut hecs::World,
        cmd: &ivy_vulkan::commands::CommandBuffer,
        _current_frame: usize,
        resources: &ivy_resources::Resources,
    ) -> anyhow::Result<()> {
        let swapchain = resources.get(self.swapchain)?;

        let image_index = swapchain
            .image_index()
            .context("Failed to get image index from swapchain")?;

        let dst = resources.get(self.swapchain_images[image_index as usize])?;

        let src = resources.get(self.read_attachments[0])?;

        let dst_barrier = vk::ImageMemoryBarrier {
            image: dst.image(),
            ..self.dst_barrier
        };

        cmd.pipeline_barrier(
            PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
            PipelineStageFlags::TRANSFER,
            &[dst_barrier],
        );

        cmd.copy_image(
            src.image(),
            ImageLayout::TRANSFER_SRC_OPTIMAL,
            dst.image(),
            ImageLayout::TRANSFER_DST_OPTIMAL,
            &[ImageCopy {
                src_subresource: ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                src_offset: vk::Offset3D::default(),
                dst_subresource: ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: 0,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                dst_offset: vk::Offset3D::default(),
                extent: vk::Extent3D {
                    width: swapchain.extent().width,
                    height: swapchain.extent().height,
                    depth: 1,
                },
            }],
        );

        let barrier = vk::ImageMemoryBarrier {
            image: dst.image(),
            ..self.output_barrier
        };

        cmd.pipeline_barrier(
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::BOTTOM_OF_PIPE,
            &[barrier],
        );

        Ok(())
    }
}
