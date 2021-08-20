use crate::{Error, Node, NodeKind, Result};
use hash::Hash;
use hecs::World;
// use itertools::Itertools;
use ivy_resources::{Handle, ResourceCache, Resources};
use ivy_vulkan::{
    commands::{CommandBuffer, CommandPool},
    fence, semaphore,
    vk::{self, CommandBufferUsageFlags, ImageMemoryBarrier, PipelineStageFlags, Semaphore},
    AttachmentDescription, AttachmentReference, Extent, Fence, Framebuffer, ImageLayout, LoadOp,
    RenderPass, RenderPassInfo, StoreOp, SubpassDependency, SubpassInfo, Texture, VulkanContext,
};
use slotmap::{new_key_type, SecondaryMap, SlotMap};
use std::{hash, ops::Deref, sync::Arc};

new_key_type! {
    pub struct NodeIndex;
    pub struct PassIndex;
}

/// Direct acyclic graph abstraction for renderpasses, barriers and subpass dependencies.
pub struct RenderGraph {
    context: Arc<VulkanContext>,
    /// The unordered nodes in the arena.
    nodes: SlotMap<NodeIndex, Box<dyn Node>>,
    edges: SecondaryMap<NodeIndex, Vec<Edge>>,
    dependencies: SecondaryMap<NodeIndex, Vec<Edge>>,
    passes: SlotMap<PassIndex, Pass>,
    // Maps a node to a pass index
    node_pass_map: SecondaryMap<NodeIndex, PassIndex>,

    // Data for each frame in flight
    frames: Vec<FrameData>,
    extent: Extent,
    frames_in_flight: usize,
    current_frame: usize,
}

impl RenderGraph {
    /// Creates a new empty rendergraph.
    pub fn new(context: Arc<VulkanContext>, frames_in_flight: usize) -> Result<Self> {
        let frames = (0..frames_in_flight)
            .map(|_| FrameData::new(context.clone()).map_err(|e| e.into()))
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            context,
            nodes: SlotMap::with_key(),
            edges: SecondaryMap::new(),
            dependencies: SecondaryMap::new(),
            passes: SlotMap::with_key(),
            node_pass_map: SecondaryMap::new(),
            frames,
            extent: Extent::new(0, 0),
            frames_in_flight,
            current_frame: 0,
        })
    }

    /// Adds a new node into the rendergraph.
    /// **Note**: The new node won't take effect until [`RenderGraph::Build`] is called.
    pub fn add_node<T: 'static + Node>(&mut self, node: T) -> NodeIndex {
        self.nodes.insert(Box::new(node))
    }

    pub fn build_edges(&mut self) -> Result<()> {
        // Clear edges
        self.edges.clear();

        // Iterate all node's read attachments, and find any node which has the same write
        // attachment before the current node.
        // Finally, automatically construct edges.
        let nodes = &self.nodes;
        let edges = &mut self.edges;
        let dependencies = &mut self.dependencies;
        nodes
            .iter()
            .flat_map(|(dst, dst_node)| {
                // Find the corresponding write attachment
                dst_node.read_attachments().iter().map(move |read| {
                    nodes.iter().find_map(|(src, src_node)| {
                        // Found color attachment output
                        if let Some(write) = src_node
                            .color_attachments()
                            .iter()
                            .find(|w| w.resource == *read)
                        {
                            Some(Edge {
                                src,
                                dst,
                                resource: *read,
                                layout: write.final_layout,
                                write_stage: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                                read_stage: vk::PipelineStageFlags::FRAGMENT_SHADER,
                                write_access: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                                read_access: vk::AccessFlags::SHADER_READ,
                            })
                            // Found depth attachment output
                        } else if let Some(write) = src_node
                            .depth_attachment()
                            .as_ref()
                            .filter(|d| d.resource == *read)
                        {
                            Some(Edge {
                                src,
                                dst,
                                resource: *read,
                                layout: write.final_layout,
                                // Write stage is between
                                write_stage: vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                                read_stage: vk::PipelineStageFlags::FRAGMENT_SHADER,
                                write_access: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                                read_access: vk::AccessFlags::SHADER_READ,
                            })
                        } else {
                            None
                        }
                    })
                })
            })
            .map(|val| val.ok_or(Error::MissingWrite))
            .try_for_each(|entry| -> Result<_> {
                let entry = entry?;
                edges
                    .entry(entry.src)
                    .unwrap()
                    .or_insert_with(Vec::new)
                    .push(entry);

                dependencies
                    .entry(entry.dst)
                    .unwrap()
                    .or_insert_with(Vec::new)
                    .push(entry);

                Ok(())
            })
    }

    /// Adds a new dependency between two nodes by introducing an edge.
    pub fn add_edge(&mut self, src: NodeIndex, edge: Edge) {
        self.edges
            .entry(src)
            .unwrap()
            .or_insert_with(Vec::new)
            .push(edge);
    }

    pub fn node_renderpass<'a>(&'a self, node_index: NodeIndex) -> Result<&'a RenderPass> {
        let pass = self
            .node_pass_map
            .get(node_index)
            .and_then(|index| self.passes.get(*index))
            .ok_or(Error::InvalidNodeIndex(node_index))?;

        match &pass.kind {
            PassKind::Graphics {
                renderpass,
                framebuffer: _,
            } => Ok(&renderpass),
            PassKind::Transfer { .. } => Err(Error::InvalidNodeKind(
                node_index,
                NodeKind::Graphics,
                self.nodes[node_index].node_kind(),
            )),
        }
    }

    /// Builds or rebuilds the rendergraph and creates appropriate renderpasses and framebuffers.
    pub fn build<T>(&mut self, textures: T, extent: Extent) -> Result<()>
    where
        T: Deref<Target = ResourceCache<Texture>>,
    {
        self.build_edges()?;
        let (ordered, _depths) = topological_sort(&self.nodes, &self.edges)?;

        let context = &self.context;

        let nodes = &self.nodes;

        self.node_pass_map.clear();

        let node_pass_map = &mut self.node_pass_map;
        node_pass_map.clear();

        let passes = &mut self.passes;
        passes.clear();

        let dependencies = &self.dependencies;

        // Build all graphics nodes
        ordered
            .iter()
            .enumerate()
            .map(|(i, node_index)| (i, *node_index, nodes.get(*node_index).unwrap()))
            .try_for_each(|(i, node_index, node)| -> Result<_> {
                let pass = Pass::new(
                    context,
                    nodes,
                    &textures,
                    dependencies,
                    &ordered[i..],
                    node.node_kind(),
                    extent,
                )?;

                let pass_index = passes.insert(pass);

                node_pass_map.insert(node_index, pass_index);

                Ok(())
            })?;

        self.extent = extent;

        Ok(())
    }

    // Begins the current frame and ensures resources are ready by waiting on fences.
    // Begins recording of the commandbuffers.
    // Returns the current frame in flight
    pub fn begin(&self) -> Result<usize> {
        let frame = &self.frames[self.current_frame];
        let device = self.context.device();

        // Make sure frame is available before beginning execution
        fence::wait(device, &[frame.fence], true)?;
        fence::reset(device, &[frame.fence])?;

        // Reset commandbuffers for this frame
        frame.commandpool.reset(false)?;

        // Get the commandbuffer for this frame
        let commandbuffer = &frame.commandbuffer;

        // Start recording
        commandbuffer.begin(CommandBufferUsageFlags::ONE_TIME_SUBMIT)?;

        Ok(self.current_frame)
    }

    // Executes the whole rendergraph by starting renderpass recording and filling it using the
    // node execution functions. Submits the resulting commandbuffer.
    pub fn execute(&mut self, world: &mut World, resources: &Resources) -> Result<()> {
        // Reset all commandbuffers for this frame
        let frame = &mut self.frames[self.current_frame];

        let nodes = &mut self.nodes;
        let passes = &self.passes;
        let extent = self.extent;
        let current_frame = self.current_frame;

        let cmd = &frame.commandbuffer;

        // Execute all nodes
        passes
            .iter()
            .try_for_each(|(_pass_index, pass)| -> Result<()> {
                pass.execute(world, &cmd, nodes, current_frame, resources, extent)
            })?;

        Ok(())
    }

    /// Ends and submits recording of commandbuffer for the current frame, and increments the
    /// current_frame.
    pub fn end(&mut self) -> Result<()> {
        let frame = &self.frames[self.current_frame];
        let commandbuffer = &frame.commandbuffer;
        commandbuffer.end()?;

        // Submit the results
        commandbuffer.submit(
            self.context.graphics_queue(),
            &[frame.wait_semaphore],
            &[frame.signal_semaphore],
            frame.fence,
            &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT],
        )?;

        // Move to the next frame in flight and wrap around to n-buffer
        self.current_frame = (self.current_frame + 1) % self.frames_in_flight;

        Ok(())
    }

    /// Get a reference to the current signal semaphore for the specified frame
    pub fn signal_semaphore(&self, current_frame: usize) -> Semaphore {
        self.frames[current_frame].signal_semaphore
    }

    /// Get a reference to the current wait semaphore for the specified frame
    pub fn wait_semaphore(&self, current_frame: usize) -> Semaphore {
        self.frames[current_frame].wait_semaphore
    }

    /// Get a reference to the current fence for the specified frame
    pub fn fence(&self, current_frame: usize) -> Fence {
        self.frames[current_frame].fence
    }

    pub fn commandbuffer(&self, current_frame: usize) -> &CommandBuffer {
        &self.frames[current_frame].commandbuffer
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum VisitedState {
    Pending,
    Visited,
}

type Depth = u32;

/// Toplogically sorts the graph provided by nodes and edges.
/// Returns a tuple containing a dense array of ordered node indices, and a map containing each node's maximum depth.
// TODO: Move graph functionality into separate crate.
fn topological_sort<T, N>(
    nodes: N,
    edges: &SecondaryMap<NodeIndex, Vec<Edge>>,
) -> Result<(Vec<NodeIndex>, SecondaryMap<NodeIndex, Depth>)>
where
    N: IntoIterator<Item = (NodeIndex, T)>,
{
    fn internal(
        stack: &mut Vec<NodeIndex>,
        visited: &mut SecondaryMap<NodeIndex, VisitedState>,
        depths: &mut SecondaryMap<NodeIndex, Depth>,
        current_node: NodeIndex,
        edges: &SecondaryMap<NodeIndex, Vec<Edge>>,
        depth: Depth,
    ) -> Result<()> {
        // Update maximum recursion depth
        depths
            .entry(current_node)
            .unwrap()
            .and_modify(|d| *d = (*d).max(depth))
            .or_insert(depth);

        // Node is already visited
        match visited.get(current_node) {
            Some(VisitedState::Pending) => return Err(Error::DependencyCycle),
            Some(VisitedState::Visited) => return Ok(()),
            _ => {}
        };

        visited.insert(current_node, VisitedState::Pending);

        // Add all children of `node`, before node to the stack.
        edges
            .get(current_node)
            .iter()
            .flat_map(|node_edges| node_edges.iter())
            .try_for_each(|edge| internal(stack, visited, depths, edge.dst, edges, depth + 1))?;

        stack.push(current_node);

        visited.insert(current_node, VisitedState::Visited);
        Ok(())
    }

    let mut nodes_iter = nodes.into_iter();
    let cap = nodes_iter.size_hint().1.unwrap_or_default();
    let mut stack = Vec::with_capacity(cap);
    let mut visited = SecondaryMap::with_capacity(cap);
    let mut depths = SecondaryMap::with_capacity(cap);

    loop {
        if let Some(node) = nodes_iter.next().map(|(i, _)| i) {
            internal(&mut stack, &mut visited, &mut depths, node, edges, 0)?;
        } else {
            break;
        }
    }

    stack.reverse();

    Ok((stack, depths))
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
pub struct Edge {
    pub src: NodeIndex,
    pub dst: NodeIndex,
    pub resource: Handle<Texture>,
    pub write_stage: PipelineStageFlags,
    pub read_stage: PipelineStageFlags,
    pub write_access: vk::AccessFlags,
    pub read_access: vk::AccessFlags,
    pub layout: ImageLayout,
}

struct FrameData {
    context: Arc<VulkanContext>,
    fence: Fence,
    commandpool: CommandPool,
    commandbuffer: CommandBuffer,
    wait_semaphore: Semaphore,
    signal_semaphore: Semaphore,
}

impl FrameData {
    pub fn new(context: Arc<VulkanContext>) -> Result<Self> {
        let commandpool = CommandPool::new(
            context.device().clone(),
            context.queue_families().graphics().unwrap(),
            true,
            false,
        )?;

        let commandbuffer = commandpool.allocate_one()?;
        let fence = fence::create(context.device(), true)?;

        let wait_semaphore = semaphore::create(context.device())?;
        let signal_semaphore = semaphore::create(context.device())?;

        Ok(Self {
            context,
            fence,
            commandpool,
            commandbuffer,
            wait_semaphore,
            signal_semaphore,
        })
    }
}

impl Drop for FrameData {
    fn drop(&mut self) {
        let device = self.context.device();

        fence::destroy(device, self.fence);
        semaphore::destroy(device, self.wait_semaphore);
        semaphore::destroy(device, self.signal_semaphore);
    }
}

struct Pass {
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
}

enum PassKind {
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
                    format: texture.format(),
                    samples: texture.samples(),
                    store_op: attachment.store_op,
                    load_op: attachment.load_op,
                    initial_layout: attachment.initial_layout,
                    final_layout: attachment.final_layout,
                    stencil_load_op: LoadOp::DONT_CARE,
                    stencil_store_op: StoreOp::DONT_CARE,
                    flags: vk::AttachmentDescriptionFlags::default(),
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
        context: &Arc<VulkanContext>,
        nodes: &SlotMap<NodeIndex, Box<dyn Node>>,
        textures: &ResourceCache<Texture>,
        dependencies: &SecondaryMap<NodeIndex, Vec<Edge>>,
        ordered_nodes: &[NodeIndex],
        extent: Extent,
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
                    src_queue_family_index: context.queue_families().graphics().unwrap(),
                    dst_queue_family_index: context.queue_families().transfer().unwrap(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topological_sort() {
        let mut nodes = SlotMap::with_key();

        let a = nodes.insert('a');
        let b = nodes.insert('b');
        let c = nodes.insert('c');
        let d = nodes.insert('d');
        let e = nodes.insert('e');

        let mut edges = SecondaryMap::new();
        // edges.insert(a, vec![c]);
        // edges.insert(b, vec![a, c]);
        // // edges.insert(e, vec![d]);
        // edges.insert(c, vec![e]);
        // edges.insert(d, vec![a]);

        edges.insert(
            a,
            vec![Edge {
                src: a,
                dst: c,
                ..Default::default()
            }],
        );
        edges.insert(
            b,
            vec![
                Edge {
                    src: b,
                    dst: a,
                    ..Default::default()
                },
                Edge {
                    src: b,
                    dst: c,
                    ..Default::default()
                },
            ],
        );
        // edges.insert(e, vec![Edge { src: a, dst: d, write_stage: PipelineStageFlags::VERTEX_SHADER, read_stage: PipelineStageFlags::FRAGMENT_SHADER }]);
        edges.insert(
            c,
            vec![Edge {
                src: c,
                dst: e,
                ..Default::default()
            }],
        );
        edges.insert(
            d,
            vec![Edge {
                src: d,
                dst: a,
                ..Default::default()
            }],
        );

        let (ordered, depths) =
            topological_sort(&nodes, &edges).expect("Failed to build rendergraph");

        let ordered_nodes = ordered
            .iter()
            .map(|node_idx| (nodes[*node_idx], depths[*node_idx]))
            .collect::<Vec<_>>();

        dbg!(ordered_nodes);
        assert_eq!(&ordered, &[d, b, a, c, e,]);
    }

    #[test]
    fn test_sort_cyclic() {
        let mut nodes = SlotMap::with_key();

        let a = nodes.insert('a');
        let b = nodes.insert('b');
        let c = nodes.insert('c');
        let d = nodes.insert('d');
        let e = nodes.insert('e');

        let mut edges = SecondaryMap::new();

        edges.insert(
            a,
            vec![Edge {
                src: a,
                dst: c,
                ..Default::default()
            }],
        );
        edges.insert(
            b,
            vec![
                Edge {
                    src: a,
                    dst: a,
                    ..Default::default()
                },
                Edge {
                    src: a,
                    dst: c,
                    ..Default::default()
                },
            ],
        );
        edges.insert(
            e,
            vec![Edge {
                src: a,
                dst: d,
                ..Default::default()
            }],
        );
        edges.insert(
            c,
            vec![Edge {
                src: a,
                dst: e,
                ..Default::default()
            }],
        );
        edges.insert(
            d,
            vec![Edge {
                src: a,
                dst: a,
                ..Default::default()
            }],
        );

        assert!(
            matches!(
                topological_sort(&nodes, &edges),
                Err(Error::DependencyCycle)
            ),
            "Did not detected cyclic graph"
        );
    }
}
