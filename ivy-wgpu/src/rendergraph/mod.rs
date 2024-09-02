mod resources;
use flax::World;
use ivy_assets::AssetCache;
pub use resources::*;
use slotmap::SecondaryMap;

use std::{
    collections::{BTreeSet, HashMap},
    mem,
};

use itertools::Itertools;
use ivy_wgpu_types::Gpu;
use wgpu::{Buffer, BufferUsages, CommandEncoder, Queue, Texture, TextureUsages};

pub struct NodeExecutionContext<'a> {
    pub gpu: &'a Gpu,
    pub resources: &'a Resources,
    pub queue: &'a Queue,
    pub encoder: &'a mut CommandEncoder,
    pub assets: &'a AssetCache,
    pub world: &'a mut World,
    external_resources: &'a ExternalResources<'a>,
}

impl<'a> NodeExecutionContext<'a> {
    pub fn get_texture(&self, handle: TextureHandle) -> &'a Texture {
        match self.external_resources.external_textures.get(handle) {
            Some(v) => v,
            None => self.resources.get_texture_data(handle),
        }
    }

    pub fn get_buffer(&self, handle: BufferHandle) -> &'a Buffer {
        self.resources.get_buffer_data(handle)
    }
}

pub struct NodeUpdateContext<'a> {
    pub gpu: &'a Gpu,
    pub resources: &'a Resources,
    pub assets: &'a AssetCache,
    pub world: &'a mut World,
    external_resources: &'a ExternalResources<'a>,
}

impl<'a> NodeUpdateContext<'a> {
    pub fn get_texture(&self, handle: TextureHandle) -> &'a Texture {
        match self.external_resources.external_textures.get(handle) {
            Some(v) => v,
            None => self.resources.get_texture_data(handle),
        }
    }

    pub fn get_buffer(&self, handle: BufferHandle) -> &'a Buffer {
        self.resources.get_buffer_data(handle)
    }
}
pub trait Node: 'static {
    fn label(&self) -> &str {
        std::any::type_name::<Self>()
    }

    fn update(&mut self, _ctx: NodeUpdateContext) -> anyhow::Result<()> {
        Ok(())
    }

    fn draw(&mut self, ctx: NodeExecutionContext) -> anyhow::Result<()>;

    fn on_resource_changed(&mut self, _resource: ResourceHandle);

    fn read_dependencies(&self) -> Vec<Dependency>;
    fn write_dependencies(&self) -> Vec<Dependency>;
}

#[derive(Debug, Clone)]
pub enum Dependency {
    Texture {
        handle: TextureHandle,
        usage: TextureUsages,
    },
    Buffer {
        handle: BufferHandle,
        usage: BufferUsages,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ResourceHandle {
    Texture(TextureHandle),
    Buffer(BufferHandle),
}

impl From<BufferHandle> for ResourceHandle {
    fn from(v: BufferHandle) -> Self {
        Self::Buffer(v)
    }
}

impl From<TextureHandle> for ResourceHandle {
    fn from(v: TextureHandle) -> Self {
        Self::Texture(v)
    }
}

impl ResourceHandle {
    pub fn as_texture(&self) -> Option<&TextureHandle> {
        if let Self::Texture(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_buffer(&self) -> Option<&BufferHandle> {
        if let Self::Buffer(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

impl Dependency {
    pub fn texture(handle: TextureHandle, usage: TextureUsages) -> Self {
        Self::Texture { handle, usage }
    }

    pub fn buffer(handle: BufferHandle, usage: BufferUsages) -> Self {
        Self::Buffer { handle, usage }
    }

    pub fn as_handle(&self) -> ResourceHandle {
        match self {
            Dependency::Texture { handle, .. } => (*handle).into(),
            Dependency::Buffer { handle, .. } => (*handle).into(),
        }
    }
}

pub struct RenderGraph {
    nodes: Vec<Box<dyn Node>>,
    order: Option<Vec<usize>>,
    pub resources: Resources,
    expected_lifetimes: HashMap<ResourceHandle, Lifetime>,

    resource_map: HashMap<ResourceHandle, BTreeSet<usize>>,
}

impl RenderGraph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            order: None,
            resources: Default::default(),
            expected_lifetimes: Default::default(),
            resource_map: Default::default(),
        }
    }

    pub fn add_node(&mut self, node: impl Node) {
        self.nodes.push(Box::new(node));
    }

    fn allocate_resources(&mut self, gpu: &Gpu) {
        self.resources
            .allocate_textures(&self.nodes, gpu, &self.expected_lifetimes);

        self.resources
            .allocate_buffers(&self.nodes, gpu, &self.expected_lifetimes);
    }

    fn build(&mut self) -> anyhow::Result<()> {
        self.resource_map.clear();
        let mut writes = HashMap::new();
        let mut reads = HashMap::new();

        for (idx, node) in self.nodes.iter().enumerate() {
            for write in node.write_dependencies() {
                self.resource_map
                    .entry(write.as_handle())
                    .or_default()
                    .insert(idx);

                if writes.insert(write.as_handle(), idx).is_some() {
                    anyhow::bail!(
                        "Multiple write dependencies for resource {:?}",
                        write.as_handle()
                    )
                }
            }

            for read in node.read_dependencies() {
                self.resource_map
                    .entry(read.as_handle())
                    .or_default()
                    .insert(idx);

                reads
                    .entry(read.as_handle())
                    .or_insert_with(Vec::new)
                    .push(idx)
            }
        }

        let dependencies = self
            .nodes
            .iter()
            .enumerate()
            .flat_map(|(node_idx, node)| {
                let writes = &writes;
                node.read_dependencies().into_iter().filter_map(move |v| {
                    let Some(&write_idx) = writes.get(&v.as_handle()) else {
                        tracing::warn!("No corresponding write found for dependency: {v:?}");
                        return None;
                    };

                    Some((node_idx, write_idx))
                })
            })
            .into_group_map();

        let TopoResult {
            order,
            dependency_levels,
        } = topo_sort(&self.nodes, &dependencies);

        self.expected_lifetimes.clear();

        for (resource, node) in writes {
            let reads = reads.get(&resource).map(Vec::as_slice).unwrap_or_default();

            let open = dependency_levels[node];
            let close = reads
                .iter()
                .map(|&node| {
                    let dep_level = dependency_levels[node];
                    assert!(dep_level > open);
                    dep_level
                })
                .max()
                .unwrap_or(open);

            tracing::info!(?resource, lifetime = ?open..close, "lifetime");
            self.expected_lifetimes
                .insert(resource, Lifetime::new(open, close + 1));
        }

        self.order = Some(order);

        Ok(())
    }

    pub fn draw(
        &mut self,
        gpu: &Gpu,
        queue: &Queue,
        world: &mut World,
        assets: &AssetCache,
        external_resources: &ExternalResources,
    ) -> anyhow::Result<()> {
        let _span = tracing::debug_span!("RenderGraph::draw").entered();

        if self.order.is_none() {
            self.build()?;
        }

        if mem::take(&mut self.resources.dirty) {
            self.invoke_on_resource_modified();
            self.allocate_resources(gpu);
        }

        let order = self.order.as_ref().unwrap();

        for &idx in order {
            let node = &mut self.nodes[idx];
            let _span = tracing::debug_span!("update", node=?node.label()).entered();
            node.update(NodeUpdateContext {
                gpu,
                resources: &self.resources,
                assets,
                world,
                external_resources,
            })?;
        }

        let mut encoder = gpu.device.create_command_encoder(&Default::default());

        for &idx in order {
            let node = &mut self.nodes[idx];
            let _span = tracing::debug_span!("draw", node=?node.label()).entered();
            node.draw(NodeExecutionContext {
                gpu,
                resources: &self.resources,
                queue,
                encoder: &mut encoder,
                assets,
                world,
                external_resources,
            })?;
        }

        queue.submit([encoder.finish()]);

        Ok(())
    }

    fn invoke_on_resource_modified(&mut self) {
        for modified in mem::take(&mut self.resources.modified_resources) {
            self.resource_map
                .get(&modified)
                .iter()
                .flat_map(|v| *v)
                .for_each(|&idx| {
                    self.nodes[idx].on_resource_changed(modified);
                })
        }
    }
}

impl Default for RenderGraph {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
pub struct ExternalResources<'a> {
    external_textures: SecondaryMap<TextureHandle, &'a Texture>,
}

impl<'a> ExternalResources<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn insert_texture(&mut self, handle: TextureHandle, texture: &'a Texture) {
        self.external_textures.insert(handle, texture);
    }
}

struct TopoResult {
    order: Vec<usize>,
    dependency_levels: Vec<u32>,
}

fn topo_sort(nodes: &[Box<dyn Node>], edges: &HashMap<usize, Vec<usize>>) -> TopoResult {
    let mut visited = vec![VisitedState::None; nodes.len()];
    let mut result = Vec::new();

    let mut dependency_levels = vec![0u32; nodes.len()];

    #[derive(Clone, Copy)]
    enum VisitedState {
        None,
        Pending,
        Visited,
    }

    fn visit(
        edges: &HashMap<usize, Vec<usize>>,
        visited: &mut Vec<VisitedState>,
        result: &mut Vec<usize>,
        dependency_levels: &mut [u32],
        node: usize,
    ) -> u32 {
        match visited[node] {
            VisitedState::None => {}
            VisitedState::Pending => {
                panic!("cyclic dependency");
            }
            VisitedState::Visited => {
                return dependency_levels[node];
            }
        }

        visited[node] = VisitedState::Pending;
        let mut max_height = 0;
        for &outgoing in edges.get(&node).into_iter().flatten() {
            max_height =
                max_height.max(visit(edges, visited, result, dependency_levels, outgoing) + 1);
        }

        visited[node] = VisitedState::Visited;

        dependency_levels[node] = max_height;
        result.push(node);
        max_height
    }

    for node in 0..nodes.len() {
        visit(
            edges,
            &mut visited,
            &mut result,
            &mut dependency_levels,
            node,
        );
    }

    TopoResult {
        order: result,
        dependency_levels,
    }
}

#[cfg(test)]
mod test {
    use ivy_wgpu_types::{Gpu, TypedBuffer};
    use wgpu::{
        BufferUsages, Extent3d, ImageCopyBuffer, ImageCopyTexture, ImageDataLayout,
        TextureDimension, TextureFormat, TextureUsages,
    };

    use crate::rendergraph::{
        BufferDesc, BufferHandle, Dependency, ManagedTextureDesc, Node, NodeExecutionContext,
        RenderGraph, TextureHandle,
    };

    #[test]
    fn write_read() {
        tracing_subscriber::fmt::init();

        let gpu = futures::executor::block_on(Gpu::headless());

        struct WriteToTexture {
            buffer: TypedBuffer<u8>,
            write: TextureHandle,
        }

        impl WriteToTexture {
            fn new(buffer: TypedBuffer<u8>, write: TextureHandle) -> Self {
                Self { buffer, write }
            }
        }

        impl Node for WriteToTexture {
            fn label(&self) -> &str {
                "WriteToTexture"
            }

            fn draw(&mut self, ctx: NodeExecutionContext) -> anyhow::Result<()> {
                let texture = ctx.resources.get_texture_data(self.write);

                ctx.encoder.copy_buffer_to_texture(
                    ImageCopyBuffer {
                        buffer: &self.buffer,
                        layout: ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(256),
                            rows_per_image: Some(256),
                        },
                    },
                    ImageCopyTexture {
                        texture,
                        mip_level: 0,
                        origin: Default::default(),
                        aspect: wgpu::TextureAspect::All,
                    },
                    Extent3d {
                        width: 256,
                        height: 256,
                        depth_or_array_layers: 1,
                    },
                );

                Ok(())
            }

            fn read_dependencies(&self) -> Vec<Dependency> {
                vec![]
            }

            fn write_dependencies(&self) -> Vec<Dependency> {
                vec![Dependency::texture(self.write, TextureUsages::COPY_DST)]
            }

            fn update(&mut self, _ctx: super::NodeUpdateContext) -> anyhow::Result<()> {
                Ok(())
            }

            fn on_resource_changed(&mut self, _resource: super::ResourceHandle) {}
        }

        struct ReadFromTexture {
            read_texture: TextureHandle,
            write_buffer: BufferHandle,
        }

        impl ReadFromTexture {
            fn new(read_texture: TextureHandle, write_buffer: BufferHandle) -> Self {
                Self {
                    read_texture,
                    write_buffer,
                }
            }
        }

        impl Node for ReadFromTexture {
            fn label(&self) -> &str {
                "ReadFromTexture"
            }

            fn draw(&mut self, ctx: NodeExecutionContext) -> anyhow::Result<()> {
                let texture = ctx.resources.get_texture_data(self.read_texture);
                let buffer = ctx.resources.get_buffer_data(self.write_buffer);

                ctx.encoder.copy_texture_to_buffer(
                    ImageCopyTexture {
                        texture,
                        mip_level: 0,
                        origin: Default::default(),
                        aspect: wgpu::TextureAspect::All,
                    },
                    ImageCopyBuffer {
                        buffer,
                        layout: ImageDataLayout {
                            offset: 0,
                            bytes_per_row: Some(256),
                            rows_per_image: Some(256),
                        },
                    },
                    Extent3d {
                        width: 256,
                        height: 256,
                        depth_or_array_layers: 1,
                    },
                );

                Ok(())
            }

            fn read_dependencies(&self) -> Vec<Dependency> {
                vec![Dependency::texture(
                    self.read_texture,
                    TextureUsages::COPY_SRC,
                )]
            }

            fn write_dependencies(&self) -> Vec<Dependency> {
                vec![Dependency::buffer(
                    self.write_buffer,
                    BufferUsages::COPY_DST,
                )]
            }

            fn update(&mut self, _ctx: super::NodeUpdateContext) -> anyhow::Result<()> {
                Ok(())
            }

            fn on_resource_changed(&mut self, _resource: super::ResourceHandle) {}
        }

        struct WriteIntoTexture {
            buffer: BufferHandle,
            texture: TextureHandle,
        }

        impl WriteIntoTexture {
            fn new(buffer: BufferHandle, texture: TextureHandle) -> Self {
                Self { buffer, texture }
            }
        }

        impl Node for WriteIntoTexture {
            fn label(&self) -> &str {
                "PostProcess"
            }

            fn draw(&mut self, ctx: NodeExecutionContext) -> anyhow::Result<()> {
                let _texture = ctx.resources.get_texture_data(self.texture);
                let _buffer = ctx.resources.get_buffer_data(self.buffer);

                Ok(())
            }

            fn read_dependencies(&self) -> Vec<Dependency> {
                vec![Dependency::buffer(self.buffer, BufferUsages::MAP_READ)]
            }

            fn write_dependencies(&self) -> Vec<Dependency> {
                vec![Dependency::texture(
                    self.texture,
                    TextureUsages::COPY_DST | TextureUsages::COPY_SRC,
                )]
            }

            fn update(&mut self, _ctx: super::NodeUpdateContext) -> anyhow::Result<()> {
                Ok(())
            }

            fn on_resource_changed(&mut self, _resource: super::ResourceHandle) {}
        }

        let mut render_graph = RenderGraph::new();

        let extent = Extent3d {
            width: 256,
            height: 256,
            depth_or_array_layers: 1,
        };

        let texture = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "src_texture".into(),
            extent,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8Uint,
            mip_level_count: 1,
            sample_count: 1,
            persistent: false,
        });

        let texture2 = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "texture_2".into(),
            extent,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8Uint,
            mip_level_count: 1,
            sample_count: 1,
            persistent: false,
        });

        let buffer = render_graph.resources.insert_buffer(BufferDesc {
            label: "dst_buffer".into(),
            size: 256 * 256,
            usage: BufferUsages::MAP_READ,
        });

        render_graph.add_node(ReadFromTexture::new(texture, buffer));

        render_graph.add_node(WriteToTexture::new(
            TypedBuffer::new(
                &gpu,
                "src_buffer",
                BufferUsages::COPY_SRC,
                &[10u8; 256 * 256],
            ),
            texture,
        ));

        render_graph.add_node(WriteIntoTexture::new(buffer, texture2));

        render_graph
            .draw(
                &gpu,
                &gpu.queue,
                &mut Default::default(),
                &Default::default(),
                &Default::default(),
            )
            .unwrap();

        let (ready_tx, ready_rx) = futures::channel::oneshot::channel();

        let buffer_data = render_graph.resources.get_buffer_data(buffer);
        buffer_data
            .slice(..)
            .map_async(wgpu::MapMode::Read, |result| {
                ready_tx.send(result).unwrap();
            });

        gpu.device.poll(wgpu::MaintainBase::Wait);

        futures::executor::block_on(ready_rx).unwrap().unwrap();
        let mapped = &*buffer_data.slice(..).get_mapped_range();

        assert_eq!(mapped[5], 10u8);
    }
}
