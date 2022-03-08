use crate::{Edge, EdgeKind, Node, NodeIndex, NodeKind, ResourceKind, Result};
use hecs::World;
use itertools::Itertools;
use ivy_base::Extent;
use ivy_resources::{ResourceCache, Resources};
use ivy_vulkan::{
    commands::CommandBuffer,
    context::SharedVulkanContext,
    vk::{self, ClearValue, ImageMemoryBarrier},
    AttachmentDescription, AttachmentReference, Framebuffer, ImageLayout, LoadOp, PassInfo,
    RenderPass, RenderPassInfo, StoreOp, SubpassDependency, SubpassInfo, Texture,
};
use std::{collections::HashMap, iter::repeat, ops::Deref};

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
        context: &SharedVulkanContext,
        nodes: &Vec<Box<dyn Node>>,
        textures: &T,
        dependencies: &HashMap<NodeIndex, Vec<Edge>>,
        pass_nodes: Vec<NodeIndex>,
        kind: NodeKind,
        extent: Extent,
    ) -> Result<Self>
    where
        T: Deref<Target = ResourceCache<Texture>>,
    {
        let kind = match kind {
            NodeKind::Graphics => {
                PassKind::graphics(context, nodes, textures, dependencies, &pass_nodes, extent)?
            }
            NodeKind::Transfer => {
                PassKind::transfer(context, nodes, textures, dependencies, &pass_nodes, extent)?
            }
        };

        Ok(Self {
            kind,
            nodes: pass_nodes,
        })
    }

    pub fn execute(
        &self,
        world: &mut World,
        cmd: &CommandBuffer,
        nodes: &mut Vec<Box<dyn Node>>,
        current_frame: usize,
        resources: &Resources,
        extent: Extent,
    ) -> Result<()> {
        match &self.kind {
            PassKind::Graphics {
                renderpass,
                framebuffer,
                clear_values,
            } => {
                cmd.begin_renderpass(&renderpass, &framebuffer, extent, clear_values);

                self.nodes
                    .iter()
                    .enumerate()
                    .try_for_each(|(subpass, index)| -> Result<_> {
                        if subpass > 0 {
                            cmd.next_subpass(vk::SubpassContents::INLINE);
                        }
                        let node = &mut nodes[*index];

                        node.execute(
                            world,
                            resources,
                            cmd,
                            &PassInfo {
                                renderpass: renderpass.renderpass(),
                                subpass: subpass as u32,
                                extent,
                                color_attachment_count: node.color_attachments().len() as u32,
                                depth_attachment: node.depth_attachment().is_some(),
                            },
                            current_frame,
                        )?;
                        Ok(())
                    })?;

                cmd.end_renderpass();
            }
            PassKind::Transfer {
                src_stage,
                image_barriers,
            } => {
                if !image_barriers.is_empty() {
                    cmd.pipeline_barrier(
                        *src_stage,
                        vk::PipelineStageFlags::TRANSFER,
                        &[],
                        image_barriers,
                    );
                }

                self.nodes.iter().try_for_each(|index| -> Result<_> {
                    nodes[*index]
                        .execute(world, resources, cmd, &PassInfo::default(), current_frame)
                        .map_err(|e| e.into())
                })?;
            }
        }

        Ok(())
    }

    /// Get a reference to the pass's kind.
    pub fn kind(&self) -> &PassKind {
        &self.kind
    }

    /// Get a reference to the pass's nodes.
    pub fn nodes(&self) -> &[NodeIndex] {
        self.nodes.as_slice()
    }
}

pub enum PassKind {
    Graphics {
        renderpass: RenderPass,
        framebuffer: Framebuffer,
        clear_values: Vec<vk::ClearValue>,
        // Index into first node of pass_nodes
    },

    Transfer {
        src_stage: vk::PipelineStageFlags,
        image_barriers: Vec<ImageMemoryBarrier>,
    },
}

unsafe impl Send for PassKind {}
unsafe impl Sync for PassKind {}

impl PassKind {
    fn graphics<T>(
        context: &SharedVulkanContext,
        nodes: &Vec<Box<dyn Node>>,
        textures: &T,
        dependencies: &HashMap<NodeIndex, Vec<Edge>>,
        pass_nodes: &[NodeIndex],
        extent: Extent,
    ) -> Result<Self>
    where
        T: Deref<Target = ResourceCache<Texture>>,
    {
        println!(
            "Building pass with nodes: {:?}",
            pass_nodes
                .iter()
                .map(|v| nodes[*v].debug_name())
                .collect_vec()
        );

        // Collect clear values
        let clear_values = pass_nodes
            .iter()
            .flat_map(|node| {
                let node = &nodes[*node];
                node.clear_values()
                    .iter()
                    .cloned()
                    .chain(repeat(ClearValue::default()).take(
                        node.color_attachments().len() + node.depth_attachment().iter().count()
                            - node.clear_values().len(),
                    ))
            })
            .collect::<Vec<_>>();

        // Generate subpass dependencies
        let dependencies = pass_nodes
            .iter()
            .enumerate()
            .flat_map(|(subpass_index, node_index)| {
                // Get the dependencies of node.
                dependencies
                    .get(node_index)
                    .into_iter()
                    .flat_map(|val| val.iter())
                    .flat_map(move |edge| match edge.kind {
                        EdgeKind::Sampled => Some(SubpassDependency {
                            src_subpass: vk::SUBPASS_EXTERNAL,
                            dst_subpass: subpass_index as u32,
                            src_stage_mask: edge.write_stage,
                            dst_stage_mask: edge.read_stage,
                            src_access_mask: edge.write_access,
                            dst_access_mask: edge.read_access,
                            dependency_flags: Default::default(),
                        }),
                        EdgeKind::Input => Some(SubpassDependency {
                            src_subpass: pass_nodes
                                .iter()
                                .enumerate()
                                .find(|(_, node)| **node == edge.src)
                                .unwrap()
                                .0 as u32,
                            dst_subpass: subpass_index as u32,
                            src_stage_mask: edge.write_stage,
                            dst_stage_mask: edge.read_stage,
                            src_access_mask: edge.write_access,
                            dst_access_mask: edge.read_access,
                            dependency_flags: vk::DependencyFlags::BY_REGION,
                        }),
                        EdgeKind::Attachment => Some(SubpassDependency {
                            src_subpass: pass_nodes
                                .iter()
                                .enumerate()
                                .find(|(_, node)| **node == edge.src)
                                .map(|v| v.0 as u32)
                                .unwrap_or(vk::SUBPASS_EXTERNAL),

                            dst_subpass: subpass_index as u32,
                            src_stage_mask: edge.write_stage,
                            dst_stage_mask: edge.read_stage,
                            src_access_mask: edge.write_access,
                            dst_access_mask: edge.read_access,
                            dependency_flags: vk::DependencyFlags::BY_REGION,
                        }),
                        EdgeKind::Buffer => None,
                    })
            })
            .collect::<Vec<_>>();

        let mut attachment_descriptions = Vec::new();
        let mut attachments = Vec::new();

        let attachment_refs = pass_nodes
            .iter()
            .enumerate()
            .map(|(_, node_index)| -> Result<_> {
                let node = &nodes[*node_index];

                let offset = attachments.len();

                let color_attachments = node
                    .color_attachments()
                    .iter()
                    .enumerate()
                    .map(|(i, _)| AttachmentReference {
                        attachment: (i + offset) as u32,
                        layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    })
                    .collect::<Vec<_>>();

                let input_attachments = node
                    .input_attachments()
                    .iter()
                    .map(|tex| -> Result<_> {
                        let view = textures.get(*tex)?.image_view();
                        Ok(AttachmentReference {
                            attachment: attachments
                                .iter()
                                .enumerate()
                                .find(|(_, val)| view == **val)
                                .unwrap()
                                .0 as u32,
                            layout: ImageLayout::SHADER_READ_ONLY_OPTIMAL,
                        })
                    })
                    .collect::<Result<Vec<_>>>()?;

                let depth_attachment =
                    node.depth_attachment()
                        .as_ref()
                        .map(|_| AttachmentReference {
                            attachment: (color_attachments.len() + input_attachments.len() + offset)
                                as u32,
                            layout: ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                        });

                for attachment in node
                    .color_attachments()
                    .iter()
                    .chain(node.depth_attachment().into_iter())
                {
                    let texture = textures.get(attachment.resource)?;

                    attachments.push(texture.image_view());

                    attachment_descriptions.push(AttachmentDescription {
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
                }
                Ok((color_attachments, input_attachments, depth_attachment))
            })
            .collect::<Result<Vec<_>>>()?;

        let subpasses = attachment_refs
            .iter()
            .map(
                |(color_attachments, input_attachments, depth_attachment)| SubpassInfo {
                    color_attachments,
                    resolve_attachments: &[],
                    input_attachments: &input_attachments,
                    depth_attachment: *depth_attachment,
                },
            )
            .collect::<Vec<_>>();

        let renderpass_info = RenderPassInfo {
            attachments: &attachment_descriptions,
            subpasses: &subpasses,
            dependencies: &dependencies,
        };

        let renderpass = RenderPass::new(context.device().clone(), &renderpass_info)?;

        let framebuffer =
            Framebuffer::new(context.device().clone(), &renderpass, &attachments, extent)?;

        Ok(PassKind::Graphics {
            renderpass,
            framebuffer,
            clear_values,
        })
    }

    fn transfer<T>(
        _context: &SharedVulkanContext,
        _nodes: &Vec<Box<dyn Node>>,
        textures: &T,
        dependencies: &HashMap<NodeIndex, Vec<Edge>>,
        pass_nodes: &[NodeIndex],
        _extent: Extent,
    ) -> Result<Self>
    where
        T: Deref<Target = ResourceCache<Texture>>,
    {
        // Get the dependencies of node.
        let mut src_stage = vk::PipelineStageFlags::default();

        let image_barriers = dependencies
            .get(&pass_nodes[0])
            .into_iter()
            .flat_map(|val| val.iter())
            .filter_map(|val| {
                if let ResourceKind::Texture(tex) = val.resource {
                    Some((val, tex))
                } else {
                    None
                }
            })
            .map(|(edge, texture)| -> Result<_> {
                let src = textures.get(texture)?;

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
