use crate::{Error, Result};
use std::{collections::HashMap, hash, ops::Deref, sync::Arc};

use hash::Hash;
use hecs::World;
use ivy_resources::{ResourceCache, Resources};
use ivy_vulkan::{
    commands::{CommandBuffer, CommandPool},
    fence, semaphore,
    vk::{self, CommandBufferUsageFlags, PipelineStageFlags, Semaphore},
    AttachmentDescription, AttachmentReference, Extent, Fence, Framebuffer, ImageLayout, LoadOp,
    RenderPass, RenderPassInfo, StoreOp, SubpassDependency, SubpassInfo, Texture, VulkanContext,
};
use slab::Slab;

use crate::NodeInfo;

pub type NodeIndex = usize;

/// Direct acyclic graph abstraction for renderpasses, barriers and subpass dependencies.
pub struct RenderGraph {
    context: Arc<VulkanContext>,
    /// The unordered nodes in the arena.
    nodes: Slab<NodeInfo>,
    edges: HashMap<NodeIndex, Vec<Edge>>,
    /// The framebuffers created from the NodeInfos. Indexed by the ordered nodes.
    framebuffers: Vec<Framebuffer>,
    /// Renderpasses in order. May or may not directly correspond to a single node as compatible
    /// nodes will be merged into subpasses. Indexed by the ordered nodes.
    renderpasses: Vec<RenderPass>,
    ordered_nodes: Vec<NodeIndex>,
    // Maps from nodes to ordered nodes
    ordered_map: HashMap<NodeIndex, usize>,

    // Data for each frame in flight
    frames: Vec<FrameData>,
    wait_semaphore: Semaphore,
    signal_semaphore: Semaphore,
    extent: Extent,
}

impl RenderGraph {
    /// Creates a new empty rendergraph.
    pub fn new(context: Arc<VulkanContext>, frames_in_flight: usize) -> Result<Self> {
        let frames = (0..frames_in_flight)
            .map(|_| FrameData::new(context.clone()).map_err(|e| e.into()))
            .collect::<Result<Vec<_>>>()?;

        let wait_semaphore = semaphore::create(context.device())?;
        let signal_semaphore = semaphore::create(context.device())?;

        Ok(Self {
            context,
            nodes: Slab::new(),
            edges: HashMap::new(),
            framebuffers: Vec::new(),
            renderpasses: Vec::new(),
            ordered_nodes: Vec::new(),
            ordered_map: HashMap::new(),
            frames,
            wait_semaphore,
            signal_semaphore,
            extent: Extent::new(0, 0),
        })
    }

    /// Adds a new node into the rendergraph.
    /// **Note**: The new node won't take effect until [`RenderGraph::Build`] is called.
    pub fn add_node(&mut self, node_info: NodeInfo) -> NodeIndex {
        self.nodes.insert(node_info)
    }

    pub fn build_edges(&mut self) -> Result<()> {
        // Clear edges
        self.edges.clear();

        // Iterate all node's read attachments, and find any node which has the same write
        // attachment before the current node.
        // Finally, automatically construct edges.
        let nodes = &self.nodes;
        let edges = &mut self.edges;
        nodes
            .iter()
            .flat_map(|(dst, dst_node)| {
                // Find the corresponding write attachment
                dst_node.read_attachments.iter().map(move |read| {
                    nodes.iter().find_map(|(src, src_node)| {
                        // Found color attachment output
                        if src_node
                            .color_attachments
                            .iter()
                            .map(|w| &w.resource)
                            .find(|w| *w == read)
                            .is_some()
                        {
                            Some(Edge {
                                src,
                                dst,
                                write_stage: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                                read_stage: vk::PipelineStageFlags::FRAGMENT_SHADER,
                                write_access: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                                read_access: vk::AccessFlags::SHADER_READ,
                            })
                        // Found depth attachment output
                        } else if src_node.depth_attachment.as_ref().map(|d| &d.resource)
                            == Some(read)
                        {
                            Some(Edge {
                                src,
                                dst,
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
                edges.entry(entry.src).or_insert_with(Vec::new).push(entry);

                Ok(())
            })
    }

    /// Adds a new dependency between two nodes by introducing an edge.
    pub fn add_edge(&mut self, src: NodeIndex, edge: Edge) {
        self.edges.entry(src).or_insert_with(Vec::new).push(edge);
    }

    pub fn node_renderpass(&self, node_index: NodeIndex) -> Result<&RenderPass> {
        let index = self
            .ordered_map
            .get(&node_index)
            .ok_or(Error::InvalidNodeIndex(node_index))?;

        Ok(&self.renderpasses[*index])
    }

    /// Builds or rebuilds the rendergraph and creates appropriate renderpasses and framebuffers.
    pub fn build<T>(&mut self, textures: T, extent: Extent) -> Result<()>
    where
        T: Deref<Target = ResourceCache<Texture>>,
    {
        self.build_edges()?;
        let (ordered, _depths) = topological_sort(&self.nodes, &self.edges)?;
        self.ordered_nodes = ordered;

        let device = self.context.device();

        let nodes = &self.nodes;

        self.ordered_map.clear();

        let ordered_map = &mut self.ordered_map;

        self.ordered_nodes
            .iter()
            .enumerate()
            .for_each(|(i, node_idx)| {
                ordered_map.insert(*node_idx, i);
            });

        // Create all renderpasses for the graph
        self.renderpasses = self
            .ordered_nodes
            .iter()
            .map(|node_idx| -> Result<RenderPass> {
                let node = &nodes[*node_idx];
                let attachments = node
                    .color_attachments
                    .iter()
                    .chain(node.depth_attachment.iter())
                    .map(|attachment| -> Result<_> {
                        let texture = textures.get(attachment.resource[0])?;
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

                let dependency = self
                    .edges
                    .get(&node_idx)
                    .iter()
                    .flat_map(|edges| edges.iter())
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
                                src_subpass: 0,
                                dst_subpass: vk::SUBPASS_EXTERNAL,
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
                    .color_attachments
                    .iter()
                    .enumerate()
                    .map(|(i, _)| AttachmentReference {
                        attachment: i as u32,
                        layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    })
                    .collect::<Vec<_>>();

                let depth_attachment =
                    node.depth_attachment.as_ref().map(|_| AttachmentReference {
                        attachment: node.color_attachments.len() as u32,
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

                let renderpass = RenderPass::new(device.clone(), &renderpass_info)?;

                Ok(renderpass)
            })
            .collect::<Result<Vec<_>>>()?;

        let renderpasses = &self.renderpasses;
        let frames_in_flight = self.frames.len();

        let textures = &textures;

        self.framebuffers = self
            .ordered_nodes
            .iter()
            .enumerate()
            .flat_map(|(i, node_idx)| {
                let node = &nodes[*node_idx];
                (0..frames_in_flight).map(move |frame| {
                    let attachments = node
                        .color_attachments
                        .iter()
                        .chain(node.depth_attachment.iter())
                        .map(|attachment| Ok(textures.get(attachment.resource[frame])?))
                        .collect::<Result<Vec<_>>>()?;
                    let framebuffer =
                        Framebuffer::new(device.clone(), &renderpasses[i], &attachments, extent)?;

                    Ok(framebuffer)
                })
            })
            .collect::<Result<Vec<_>>>()?;

        self.extent = extent;

        Ok(())
    }

    // Executes the whole rendergraph by starting renderpass recording and filling it using the
    // node execution functions. Submits the resulting commandbuffer.
    pub fn execute(
        &mut self,
        world: &mut World,
        current_frame: usize,
        resources: &Resources,
    ) -> Result<()> {
        let frame = &self.frames[current_frame];
        let device = self.context.device();

        // Make sure frame is available before beginning execution
        fence::wait(device, &[frame.fence], true)?;
        fence::reset(device, &[frame.fence])?;

        // Reset all commandbuffers for this frame
        frame.commandpool.reset(false)?;

        // Get the commandbuffer for this frame
        let commandbuffer = &frame.commandbuffer;

        // Start recording
        commandbuffer.begin(CommandBufferUsageFlags::ONE_TIME_SUBMIT)?;

        let nodes = &mut self.nodes;
        let renderpasses = &self.renderpasses;
        let framebuffers = &self.framebuffers;
        let extent = self.extent;
        let frames_in_flight = self.frames.len();

        // Execute all nodes
        self.ordered_nodes
            .iter()
            .enumerate()
            .try_for_each(|(i, node_idx)| -> Result<()> {
                let node = &mut nodes[*node_idx];

                commandbuffer.begin_renderpass(
                    &renderpasses[i],
                    &framebuffers[frames_in_flight * i + current_frame],
                    extent,
                    &node.clear_values,
                );

                node.node
                    .execute(world, &commandbuffer, current_frame, resources)?;

                commandbuffer.end_renderpass();

                Ok(())
            })?;

        commandbuffer.end()?;

        // Submit the results
        commandbuffer.submit(
            self.context.graphics_queue(),
            &[self.wait_semaphore],
            &[self.signal_semaphore],
            frame.fence,
            &[vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT],
        )?;

        Ok(())
    }

    /// Get a reference to the render graph's signal semaphore.
    pub fn signal_semaphore(&self) -> Semaphore {
        self.signal_semaphore
    }

    /// Get a reference to the render graph's wait semaphore.
    pub fn wait_semaphore(&self) -> Semaphore {
        self.wait_semaphore
    }
}
impl Drop for RenderGraph {
    fn drop(&mut self) {
        let device = self.context.device();

        semaphore::destroy(device, self.wait_semaphore);
        semaphore::destroy(device, self.signal_semaphore);
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
    edges: &HashMap<NodeIndex, Vec<Edge>>,
) -> Result<(Vec<NodeIndex>, HashMap<NodeIndex, Depth>)>
where
    N: IntoIterator<Item = (NodeIndex, T)>,
{
    fn internal(
        stack: &mut Vec<NodeIndex>,
        visited: &mut HashMap<NodeIndex, VisitedState>,
        depths: &mut HashMap<NodeIndex, Depth>,
        current_node: NodeIndex,
        edges: &HashMap<NodeIndex, Vec<Edge>>,
        depth: Depth,
    ) -> Result<()> {
        // Update maximum recursion depth
        depths
            .entry(current_node)
            .and_modify(|d| *d = (*d).max(depth))
            .or_insert(depth);

        // Node is already visited
        match visited.get(&current_node) {
            Some(VisitedState::Pending) => return Err(Error::DependencyCycle),
            Some(VisitedState::Visited) => return Ok(()),
            _ => {}
        };

        visited.insert(current_node, VisitedState::Pending);

        // Add all children of `node`, before node to the stack.
        edges
            .get(&current_node)
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
    let mut visited = HashMap::with_capacity(cap);
    let mut depths = HashMap::with_capacity(cap);

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_topological_sort() {
        let mut nodes = Slab::new();

        let a = nodes.insert('a');
        let b = nodes.insert('b');
        let c = nodes.insert('c');
        let d = nodes.insert('d');
        let e = nodes.insert('e');

        let mut edges = HashMap::new();
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
            .map(|node_idx| (nodes[*node_idx], depths[node_idx]))
            .collect::<Vec<_>>();

        dbg!(ordered_nodes);
        assert_eq!(&ordered, &[d, b, a, c, e,]);
    }

    #[test]
    fn test_sort_cyclic() {
        let mut nodes = Slab::new();

        let a = nodes.insert('a');
        let b = nodes.insert('b');
        let c = nodes.insert('c');
        let d = nodes.insert('d');
        let e = nodes.insert('e');

        let mut edges = HashMap::new();

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

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Default)]
pub struct Edge {
    pub src: NodeIndex,
    pub dst: NodeIndex,
    pub write_stage: PipelineStageFlags,
    pub read_stage: PipelineStageFlags,
    pub write_access: vk::AccessFlags,
    pub read_access: vk::AccessFlags,
}

struct FrameData {
    context: Arc<VulkanContext>,
    fence: Fence,
    commandpool: CommandPool,
    commandbuffer: CommandBuffer,
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

        Ok(Self {
            context,
            fence,
            commandpool,
            commandbuffer,
        })
    }
}

impl Drop for FrameData {
    fn drop(&mut self) {
        let device = self.context.device();

        fence::destroy(device, self.fence);
    }
}
