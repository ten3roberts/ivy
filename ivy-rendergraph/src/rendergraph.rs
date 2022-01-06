use crate::{
    pass::{Pass, PassKind},
    Error, Node, NodeKind, Result,
};
use hash::Hash;
use hecs::World;
use itertools::Itertools;
use ivy_base::Extent;
use ivy_resources::{Handle, ResourceCache, Resources};
use ivy_vulkan::{
    commands::{CommandBuffer, CommandPool},
    context::SharedVulkanContext,
    fence, semaphore,
    vk::{self, CommandBufferUsageFlags, PipelineStageFlags, Semaphore},
    Fence, ImageLayout, PipelineInfo, RenderPass, Texture,
};
use slotmap::{new_key_type, SecondaryMap, SlotMap};
use std::{hash, ops::Deref, time::Duration};

new_key_type! {
    pub struct NodeIndex;
    pub struct PassIndex;
}

/// Direct acyclic graph abstraction for renderpasses, barriers and subpass dependencies.
pub struct RenderGraph {
    context: SharedVulkanContext,
    /// The unordered nodes in the arena.
    nodes: SlotMap<NodeIndex, Box<dyn Node>>,
    edges: SecondaryMap<NodeIndex, Vec<Edge>>,
    dependencies: SecondaryMap<NodeIndex, Vec<Edge>>,
    passes: SlotMap<PassIndex, Pass>,
    // Maps a node to a pass index
    node_pass_map: SecondaryMap<NodeIndex, (PassIndex, u32)>,

    // Data for each frame in flight
    frames: Vec<FrameData>,
    extent: Extent,
    frames_in_flight: usize,
    current_frame: usize,

    execution_times: SecondaryMap<NodeIndex, (&'static str, Duration)>,
}

impl RenderGraph {
    /// Creates a new empty rendergraph.
    pub fn new(context: SharedVulkanContext, frames_in_flight: usize) -> Result<Self> {
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
            execution_times: SecondaryMap::new(),
        })
    }

    /// Adds a new node into the rendergraph.
    /// **Note**: The new node won't take effect until [`RenderGraph::build`] is called.
    pub fn add_node<T: 'static + Node>(&mut self, node: T) -> NodeIndex {
        self.nodes.insert(Box::new(node))
    }

    /// Add several nodes into the rendergraph.
    /// Due to the concrete type of iterators, the nodes need to already be boxed.
    /// Returns the node indices in order.
    pub fn add_nodes<I: IntoIterator<Item = Box<dyn Node>>>(&mut self, nodes: I) -> Vec<NodeIndex> {
        nodes
            .into_iter()
            .map(|node| self.nodes.insert(node))
            .collect_vec()
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
            .flat_map(|node| {
                EdgeConstructor {
                    nodes,
                    dst: node.0,
                    reads: node.1.input_attachments().into_iter().cloned(),
                    kind: EdgeKind::Input,
                }
                .chain(BufferEdgeConstructor {
                    nodes,
                    dst: node.0,
                    reads: node.1.buffer_reads().into_iter().cloned(),
                    kind: EdgeKind::Sampled,
                })
                .chain(EdgeConstructor {
                    nodes,
                    dst: node.0,
                    reads: node.1.read_attachments().into_iter().cloned(),
                    kind: EdgeKind::Sampled,
                })
                .chain(EdgeConstructor {
                    nodes,
                    dst: node.0,
                    reads: node
                        .1
                        .color_attachments()
                        .into_iter()
                        .map(|val| val.resource),
                    kind: EdgeKind::Attachment,
                })
                .filter(|val| !matches!(*val, Err(Error::MissingWrite(_, _, _))))
            })
            .try_for_each(|edge: Result<Edge>| -> Result<_> {
                let edge = edge?;
                edges
                    .entry(edge.src)
                    .unwrap()
                    .or_insert_with(Vec::new)
                    .push(edge);

                dependencies
                    .entry(edge.dst)
                    .unwrap()
                    .or_insert_with(Vec::new)
                    .push(edge);

                Ok(())
            })?;

        Ok(())
    }

    /// Adds a new dependency between two nodes by introducing an edge.
    pub fn add_edge(&mut self, src: NodeIndex, edge: Edge) {
        self.edges
            .entry(src)
            .unwrap()
            .or_insert_with(Vec::new)
            .push(edge);
    }

    pub fn node(&self, node: NodeIndex) -> Result<&dyn Node> {
        self.nodes
            .get(node)
            .ok_or_else(|| Error::InvalidNodeIndex(node))
            .map(|val| val.as_ref())
    }

    pub fn node_renderpass<'a>(&'a self, node: NodeIndex) -> Result<(&'a RenderPass, u32)> {
        let (pass, index) = self
            .node_pass_map
            .get(node)
            .and_then(|(pass, subpass_index)| Some((self.passes.get(*pass)?, *subpass_index)))
            .ok_or(Error::InvalidNodeIndex(node))?;

        match pass.kind() {
            PassKind::Graphics {
                renderpass,
                framebuffer: _,
                clear_values: _,
            } => Ok((renderpass, index)),
            PassKind::Transfer { .. } => Err(Error::InvalidNodeKind(
                node,
                NodeKind::Graphics,
                self.nodes[node].node_kind(),
            )),
        }
    }

    /// Returns a pipeline info compatible with the specified node
    pub fn pipeline_info(&self, node: NodeIndex) -> Result<PipelineInfo> {
        let (pass, subpass) = self.node_renderpass(node)?;
        let node = self.node(node)?;

        Ok(PipelineInfo {
            renderpass: pass.renderpass(),
            subpass,
            extent: self.extent,
            color_attachment_count: node.color_attachments().len() as u32,
            depth_attachment: node.depth_attachment().is_some(),
            ..Default::default()
        })
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
        let groups = ordered.iter().cloned().group_by(|node| {
            return (_depths[*node], nodes[*node].node_kind());
        });

        for (key, pass_nodes) in &groups {
            let pass_nodes = pass_nodes.collect::<Vec<_>>();
            let pass = Pass::new(
                context,
                nodes,
                &textures,
                dependencies,
                pass_nodes,
                key.1,
                extent,
            )?;

            // Insert pass into slotmap
            let pass_index = passes.insert(pass);

            let pass_nodes = passes[pass_index].nodes();

            // Map the node into the pass
            for (i, node) in pass_nodes.iter().enumerate() {
                node_pass_map.insert(*node, (pass_index, i as u32));
            }
        }

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
        let execution_times = &mut self.execution_times;
        let passes = &self.passes;
        let extent = self.extent;
        let current_frame = self.current_frame;

        let cmd = &frame.commandbuffer;

        // Execute all nodes
        passes
            .iter()
            .try_for_each(|(_pass_index, pass)| -> Result<()> {
                pass.execute(
                    world,
                    &cmd,
                    nodes,
                    current_frame,
                    resources,
                    extent,
                    execution_times,
                )
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

    pub fn execution_times(&self) -> &SecondaryMap<NodeIndex, (&'static str, Duration)> {
        &self.execution_times
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
            .try_for_each(|edge| {
                internal(
                    stack,
                    visited,
                    depths,
                    edge.dst,
                    edges,
                    // Break depth if sampling is required since they can't share subpasses
                    if edge.kind == EdgeKind::Sampled {
                        depth + 1
                    } else {
                        depth
                    },
                )
            })?;

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

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct Edge {
    pub src: NodeIndex,
    pub dst: NodeIndex,
    pub resource: ResourceKind,
    pub write_stage: PipelineStageFlags,
    pub read_stage: PipelineStageFlags,
    pub write_access: vk::AccessFlags,
    pub read_access: vk::AccessFlags,
    pub layout: ImageLayout,
    pub kind: EdgeKind,
}

impl std::ops::Deref for Edge {
    type Target = ResourceKind;

    fn deref(&self) -> &Self::Target {
        &self.resource
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceKind {
    Texture(Handle<Texture>),
    Buffer(vk::Buffer),
}

impl From<vk::Buffer> for ResourceKind {
    fn from(val: vk::Buffer) -> Self {
        Self::Buffer(val)
    }
}

impl From<Handle<Texture>> for ResourceKind {
    fn from(val: Handle<Texture>) -> Self {
        Self::Texture(val)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EdgeKind {
    /// Dependency is sampled and requires the whole attachment to be ready.
    Sampled,
    /// Dependency is used as input attachment and can use dependency by region.
    Input,
    /// The attachment is loaded and written to. Depend on earlier nodes
    Attachment,
}

impl Default for EdgeKind {
    fn default() -> Self {
        Self::Sampled
    }
}

struct FrameData {
    context: SharedVulkanContext,
    fence: Fence,
    commandpool: CommandPool,
    commandbuffer: CommandBuffer,
    wait_semaphore: Semaphore,
    signal_semaphore: Semaphore,
}

impl FrameData {
    pub fn new(context: SharedVulkanContext) -> Result<Self> {
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

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn test_topological_sort() {
//         let mut nodes = SlotMap::with_key();

//         let a = nodes.insert('a');
//         let b = nodes.insert('b');
//         let c = nodes.insert('c');
//         let d = nodes.insert('d');
//         let e = nodes.insert('e');

//         let mut edges = SecondaryMap::new();

//         edges.insert(
//             a,
//             vec![Edge {
//                 src: a,
//                 dst: c,
//                 ..Default::default()
//             }],
//         );
//         edges.insert(
//             b,
//             vec![
//                 Edge {
//                     src: b,
//                     dst: a,
//                     ..Default::default()
//                 },
//                 Edge {
//                     src: b,
//                     dst: c,
//                     ..Default::default()
//                 },
//             ],
//         );
//         edges.insert(
//             c,
//             vec![Edge {
//                 src: c,
//                 dst: e,
//                 ..Default::default()
//             }],
//         );
//         edges.insert(
//             d,
//             vec![Edge {
//                 src: d,
//                 dst: a,
//                 ..Default::default()
//             }],
//         );

//         let (ordered, _depths) =
//             topological_sort(&nodes, &edges).expect("Failed to build rendergraph");

//         assert_eq!(&ordered, &[d, b, a, c, e,]);
//     }

//     #[test]
//     fn test_sort_cyclic() {
//         let mut nodes = SlotMap::with_key();

//         let a = nodes.insert('a');
//         let b = nodes.insert('b');
//         let c = nodes.insert('c');
//         let d = nodes.insert('d');
//         let e = nodes.insert('e');

//         let mut edges = SecondaryMap::new();

//         edges.insert(
//             a,
//             vec![Edge {
//                 src: a,
//                 dst: c,
//                 ..Default::default()
//             }],
//         );
//         edges.insert(
//             b,
//             vec![
//                 Edge {
//                     src: a,
//                     dst: a,
//                     ..Default::default()
//                 },
//                 Edge {
//                     src: a,
//                     dst: c,
//                     ..Default::default()
//                 },
//             ],
//         );
//         edges.insert(
//             e,
//             vec![Edge {
//                 src: a,
//                 dst: d,
//                 ..Default::default()
//             }],
//         );
//         edges.insert(
//             c,
//             vec![Edge {
//                 src: a,
//                 dst: e,
//                 ..Default::default()
//             }],
//         );
//         edges.insert(
//             d,
//             vec![Edge {
//                 src: a,
//                 dst: a,
//                 ..Default::default()
//             }],
//         );

//         assert!(
//             matches!(
//                 topological_sort(&nodes, &edges),
//                 Err(Error::DependencyCycle)
//             ),
//             "Did not detected cyclic graph"
//         );
//     }
// }

struct EdgeConstructor<'a, I> {
    nodes: &'a SlotMap<NodeIndex, Box<dyn Node>>,
    dst: NodeIndex,
    reads: I,
    kind: EdgeKind,
}

impl<'a, I: Iterator<Item = Handle<Texture>>> Iterator for EdgeConstructor<'a, I> {
    type Item = Result<Edge>;

    fn next(&mut self) -> Option<Self::Item> {
        self.reads
            .next()
            // Find the corresponding write attachment
            .map(move |read| {
                self.nodes
                    .iter()
                    .take_while(|(src, _)| *src != self.dst)
                    .filter_map(|(src, src_node)| {
                        // Found color attachment output
                        if let Some(write) = src_node
                            .color_attachments()
                            .iter()
                            .find(|w| w.resource == read)
                        {
                            Some(Edge {
                                src,
                                dst: self.dst,
                                resource: read.into(),
                                layout: write.final_layout,
                                write_stage: vk::PipelineStageFlags::COLOR_ATTACHMENT_OUTPUT,
                                read_stage: vk::PipelineStageFlags::FRAGMENT_SHADER,
                                write_access: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                                read_access: vk::AccessFlags::SHADER_READ,
                                kind: self.kind,
                            })
                            // Found depth attachment output
                        } else if let Some(write) = src_node
                            .depth_attachment()
                            .as_ref()
                            .filter(|d| d.resource == read)
                        {
                            Some(Edge {
                                src,
                                dst: self.dst,
                                resource: read.into(),
                                layout: write.final_layout,
                                // Write stage is between
                                write_stage: vk::PipelineStageFlags::LATE_FRAGMENT_TESTS,
                                read_stage: vk::PipelineStageFlags::FRAGMENT_SHADER,
                                write_access: vk::AccessFlags::DEPTH_STENCIL_ATTACHMENT_WRITE,
                                read_access: vk::AccessFlags::SHADER_READ,
                                kind: self.kind,
                            })
                        } else {
                            None
                        }
                    })
                    .last()
                    .ok_or(Error::MissingWrite(
                        self.dst,
                        self.nodes[self.dst].debug_name(),
                        read.into(),
                    ))
            })
    }
}

struct BufferEdgeConstructor<'a, I> {
    nodes: &'a SlotMap<NodeIndex, Box<dyn Node>>,
    dst: NodeIndex,
    reads: I,
    kind: EdgeKind,
}

impl<'a, I: Iterator<Item = vk::Buffer>> Iterator for BufferEdgeConstructor<'a, I> {
    type Item = Result<Edge>;

    fn next(&mut self) -> Option<Self::Item> {
        self.reads
            .next()
            // Find the corresponding write attachment
            .map(move |read| {
                self.nodes
                    .iter()
                    .take_while(|(src, _)| *src != self.dst)
                    .filter_map(|(src, src_node)| {
                        // Found color attachment output
                        if let Some(_) = src_node.buffer_writes().iter().find(|w| **w == read) {
                            Some(Edge {
                                src,
                                dst: self.dst,
                                resource: read.into(),
                                layout: Default::default(),
                                write_stage: vk::PipelineStageFlags::TRANSFER,
                                read_stage: vk::PipelineStageFlags::VERTEX_SHADER,
                                write_access: vk::AccessFlags::COLOR_ATTACHMENT_WRITE,
                                read_access: vk::AccessFlags::SHADER_READ,
                                kind: self.kind,
                            })
                        } else {
                            None
                        }
                    })
                    .last()
                    .ok_or(Error::MissingWrite(
                        self.dst,
                        self.nodes[self.dst].debug_name(),
                        read.into(),
                    ))
            })
    }
}
