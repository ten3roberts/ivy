use crate::{Edge, Node, NodeIndex, NodeKind, Result};
use hecs::World;
use ivy_resources::{ResourceCache, Resources};
use ivy_vulkan::{
    commands::CommandBuffer,
    vk::{self, ImageMemoryBarrier},
    AttachmentDescription, AttachmentReference, Extent, Framebuffer, ImageLayout, LoadOp,
    RenderPass, RenderPassInfo, StoreOp, SubpassDependency, SubpassInfo, Texture, VulkanContext,
};
use slotmap::{SecondaryMap, SlotMap};
use std::{ops::Deref, sync::Arc};

pub struct Pass {
    kind: PassKind,
    nodes: Vec<NodeIndex>,
}

impl Pass {
    // Creates a new pass from a group of compatible nodes.
    // Two nodes are compatible if:
    // * They have no full read dependencies to another.
    // * Belong to the same kind and queue.
    // * Have the same dependency level.
    pub fn new<T>(
        context: &Arc<VulkanContext>,
        nodes: &SlotMap<NodeIndex, Box<dyn Node>>,
        textures: &T,
        dependencies: &SecondaryMap<NodeIndex, Vec<Edge>>,
        ordered_nodes: &[NodeIndex],
        kind: NodeKind,
        extent: Extent,
    ) -> Result<Self>
    where
        T: Deref<Target = ResourceCache<Texture>>,
    {
        let kind = match kind {
            NodeKind::Graphics => PassKind::graphics(
                context,
                nodes,
                textures,
                dependencies,
                ordered_nodes,
                extent,
            )?,
            NodeKind::Transfer => PassKind::transfer(
                context,
                nodes,
                textures,
                dependencies,
                ordered_nodes,
                extent,
            )?,
        };

        Ok(Self {
            kind,
            nodes: ordered_nodes.to_owned(),
        })
    }

    pub fn execute(
        &self,
        world: &mut World,
        cmd: &CommandBuffer,
        nodes: &mut SlotMap<NodeIndex, Box<dyn Node>>,
        current_frame: usize,
        resources: &Resources,
        extent: Extent,
    ) -> Result<()> {
        let node = &mut nodes[self.nodes[0]];

        match &self.kind {
            PassKind::Graphics {
                renderpass,
                framebuffer,
            } => {
                cmd.begin_renderpass(&renderpass, &framebuffer, extent, &node.clear_values());

                node.execute(world, cmd, current_frame, resources)?;

                cmd.end_renderpass();
            }
            PassKind::Transfer {
                src_stage,
                image_barriers,
            } => {
                cmd.pipeline_barrier(*src_stage, vk::PipelineStageFlags::TRANSFER, image_barriers);
                node.execute(world, cmd, current_frame, resources)?;
            }
        }

        Ok(())
    }

    /// Get a reference to the pass's kind.
    pub fn kind(&self) -> &PassKind {
        &self.kind
    }
}

pub enum PassKind {
    Graphics {
        renderpass: RenderPass,
        framebuffer: Framebuffer,
        // Index into first node of ordered_nodes
    },

    Transfer {
        src_stage: vk::PipelineStageFlags,
        image_barriers: Vec<ImageMemoryBarrier>,
    },
}

impl PassKind {
    fn graphics<T>(
        context: &Arc<VulkanContext>,
        nodes: &SlotMap<NodeIndex, Box<dyn Node>>,
        textures: &T,
        dependencies: &SecondaryMap<NodeIndex, Vec<Edge>>,
        ordered_nodes: &[NodeIndex],
        extent: Extent,
    ) -> Result<Self>
    where
        T: Deref<Target = ResourceCache<Texture>>,
    {
        let node = &nodes[ordered_nodes[0]];

        let attachments = node
            .color_attachments()
            .iter()
            .chain(node.depth_attachment().into_iter())
            .map(|attachment| -> Result<_> {
                let texture = textures.get(attachment.resource)?;
                Ok(AttachmentDescription {
                    flags: vk::AttachmentDescriptionFlags::default(),
                    format: texture.format(),
                    samples: texture.samples(),
                    load_op: attachment.load_op,
                    store_op: attachment.store_op,
                    stencil_load_op: LoadOp::DONT_CARE,
                    stencil_store_op: StoreOp::DONT_CARE,
                    initial_layout: attachment.initial_layout,
                    final_layout: attachment.final_layout,
                })
            })
            .collect::<Result<Vec<_>>>()?;

        // Get the dependencies of node.
        let dependency = dependencies
            .get(ordered_nodes[0])
            .into_iter()
            .flat_map(|val| val.iter())
            .fold(
                None,
                |dependency: Option<SubpassDependency>, edge| match dependency {
                    Some(dependency) => Some(SubpassDependency {
                        src_stage_mask: dependency.src_stage_mask | edge.write_stage,
                        dst_stage_mask: dependency.dst_stage_mask | edge.read_stage,
                        src_access_mask: dependency.src_access_mask | edge.write_access,
                        dst_access_mask: dependency.dst_access_mask | edge.read_access,
                        ..dependency
                    }),
                    None => Some(SubpassDependency {
                        src_subpass: vk::SUBPASS_EXTERNAL,
                        dst_subpass: 0,
                        src_stage_mask: edge.write_stage,
                        dst_stage_mask: edge.read_stage,
                        src_access_mask: edge.write_access,
                        dst_access_mask: edge.read_access,
                        dependency_flags: Default::default(),
                    }),
                },
            );

        let dependencies = [dependency.unwrap_or_default()];

        let color_attachments = node
            .color_attachments()
            .iter()
            .enumerate()
            .map(|(i, _)| AttachmentReference {
                attachment: i as u32,
                layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            })
            .collect::<Vec<_>>();

        let depth_attachment = node
            .depth_attachment()
            .as_ref()
            .map(|_| AttachmentReference {
                attachment: node.color_attachments().len() as u32,
                layout: ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            });

        let renderpass_info = RenderPassInfo {
            attachments: &attachments,
            subpasses: &[SubpassInfo {
                color_attachments: &color_attachments,
                resolve_attachments: &[],
                input_attachments: &[],
                depth_attachment,
            }],
            dependencies: if dependency.is_some() {
                &dependencies
            } else {
                &[]
            },
        };

        let renderpass = RenderPass::new(context.device().clone(), &renderpass_info)?;

        let attachments = node
            .color_attachments()
            .iter()
            .chain(node.depth_attachment().into_iter())
            .map(|attachment| Ok(textures.get(attachment.resource)?))
            .collect::<Result<Vec<_>>>()?;

        let framebuffer =
            Framebuffer::new(context.device().clone(), &renderpass, &attachments, extent)?;

        Ok(PassKind::Graphics {
            renderpass,
            framebuffer,
        })
    }

    fn transfer(
        _context: &Arc<VulkanContext>,
        _nodes: &SlotMap<NodeIndex, Box<dyn Node>>,
        textures: &ResourceCache<Texture>,
        dependencies: &SecondaryMap<NodeIndex, Vec<Edge>>,
        ordered_nodes: &[NodeIndex],
        _extent: Extent,
    ) -> Result<Self> {
        // Get the dependencies of node.
        let mut src_stage = vk::PipelineStageFlags::default();

        let image_barriers = dependencies
            .get(ordered_nodes[0])
            .into_iter()
            .flat_map(|val| val.iter())
            .map(|edge| -> Result<_> {
                let src = textures.get(edge.resource)?;

                let aspect_mask =
                    if edge.read_access == vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE {
                        vk::ImageAspectFlags::DEPTH
                    } else {
                        vk::ImageAspectFlags::COLOR
                    };
                src_stage = edge.write_stage.max(src_stage);

                Ok(ImageMemoryBarrier {
                    src_access_mask: edge.write_access,
                    dst_access_mask: vk::AccessFlags::TRANSFER_READ,
                    old_layout: edge.layout,
                    new_layout: ImageLayout::TRANSFER_SRC_OPTIMAL,
                    src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                    dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
                    image: src.image(),
                    subresource_range: vk::ImageSubresourceRange {
                        aspect_mask,
                        base_mip_level: 0,
                        level_count: src.mip_levels(),
                        base_array_layer: 0,
                        layer_count: 1,
                    },
                    ..Default::default()
                })
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self::Transfer {
            src_stage,
            image_barriers,
        })
    }
}
