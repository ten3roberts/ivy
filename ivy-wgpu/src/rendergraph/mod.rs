mod resources;
use flax::World;
use ivy_assets::AssetCache;
use ivy_core::profiling::{profile_function, profile_scope};
pub use resources::*;
use slotmap::{new_key_type, SecondaryMap, SlotMap};

use std::{
    collections::{BTreeSet, HashMap},
    mem,
};

use itertools::Itertools;
use ivy_wgpu_types::Gpu;
use wgpu::{Buffer, BufferUsages, CommandEncoder, Queue, Texture, TextureUsages};

pub struct NodeExecutionContext<'a> {
    pub gpu: &'a Gpu,
    pub resources: &'a RenderGraphResources,
    pub queue: &'a Queue,
    pub encoder: &'a mut CommandEncoder,
    pub assets: &'a AssetCache,
    pub world: &'a mut World,
    pub external_resources: &'a ExternalResources<'a>,
}

impl<'a> NodeExecutionContext<'a> {
    #[track_caller]
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
    pub resources: &'a RenderGraphResources,
    pub assets: &'a AssetCache,
    pub world: &'a mut World,
    pub external_resources: &'a ExternalResources<'a>,
}

impl<'a> NodeUpdateContext<'a> {
    #[track_caller]
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

pub enum UpdateResult {
    Success,
    RecalculateDepencies,
}

pub trait Node: 'static {
    fn label(&self) -> &str {
        std::any::type_name::<Self>()
    }

    fn update(&mut self, _ctx: NodeUpdateContext) -> anyhow::Result<UpdateResult> {
        Ok(UpdateResult::Success)
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

new_key_type! {
    pub struct NodeId;
}

pub struct RenderGraph {
    nodes: SlotMap<NodeId, Box<dyn Node>>,
    order: Option<Vec<NodeId>>,
    expected_lifetimes: HashMap<ResourceHandle, Lifetime>,

    resource_to_nodes: HashMap<ResourceHandle, BTreeSet<NodeId>>,
    pub resources: RenderGraphResources,
}

impl RenderGraph {
    pub fn new(resources: RenderGraphResources) -> Self {
        Self {
            nodes: Default::default(),
            order: None,
            expected_lifetimes: Default::default(),
            resource_to_nodes: Default::default(),
            resources,
        }
    }

    pub fn add_node(&mut self, node: impl Node) -> NodeId {
        self.order = None;
        self.nodes.insert(Box::new(node))
    }

    pub fn remove_node(&mut self, node_id: NodeId) -> Option<Box<dyn Node>> {
        self.order = None;
        self.nodes.remove(node_id)
    }

    fn allocate_resources(&mut self, gpu: &Gpu) -> anyhow::Result<()> {
        self.resources
            .allocate_textures(&self.nodes, gpu, &self.expected_lifetimes)?;
        self.resources
            .allocate_buffers(&self.nodes, gpu, &self.expected_lifetimes)?;

        Ok(())
    }

    fn build(&mut self) -> anyhow::Result<()> {
        profile_function!();

        self.resource_to_nodes.clear();
        let mut writes = HashMap::new();
        let mut reads = HashMap::new();

        for (id, node) in self.nodes.iter() {
            for write in node.write_dependencies() {
                self.resource_to_nodes
                    .entry(write.as_handle())
                    .or_default()
                    .insert(id);

                if writes.insert(write.as_handle(), id).is_some() {
                    anyhow::bail!(
                        "Multiple write dependencies for resource {:?}",
                        write.as_handle()
                    )
                }
            }

            for read in node.read_dependencies() {
                self.resource_to_nodes
                    .entry(read.as_handle())
                    .or_default()
                    .insert(id);

                reads
                    .entry(read.as_handle())
                    .or_insert_with(Vec::new)
                    .push(id)
            }
        }

        let dependencies = self
            .nodes
            .iter()
            .flat_map(|(node_id, node)| {
                let writes = &writes;
                node.read_dependencies().into_iter().filter_map(move |v| {
                    let Some(&write_idx) = writes.get(&v.as_handle()) else {
                        tracing::warn!("No corresponding write found for dependency: {v:?}");
                        return None;
                    };

                    Some((node_id, write_idx))
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

    fn rebuild(&mut self, gpu: &Gpu) -> anyhow::Result<()> {
        self.build()?;

        self.invoke_on_resource_modified();
        self.allocate_resources(gpu)?;

        Ok(())
    }

    pub fn update(
        &mut self,
        gpu: &Gpu,
        world: &mut World,
        assets: &AssetCache,
        external_resources: &ExternalResources,
    ) -> anyhow::Result<()> {
        profile_function!();

        if self.order.is_none() {
            tracing::info!("rebuilding");
            self.build()?;
        }

        if mem::take(&mut self.resources.dirty) {
            tracing::info!("dirty resources");
            self.invoke_on_resource_modified();
            self.allocate_resources(gpu)?;

            self.resources.modified_resources.clear();
        }

        let order = self.order.as_ref().unwrap();

        let mut needs_rebuild = false;
        for &idx in order {
            let node = &mut self.nodes[idx];
            profile_scope!("update_node", node.label());

            let res = node.update(NodeUpdateContext {
                gpu,
                resources: &self.resources,
                assets,
                world,
                external_resources,
            })?;

            match res {
                UpdateResult::Success => {}
                UpdateResult::RecalculateDepencies => {
                    needs_rebuild = true;
                }
            }
        }

        if needs_rebuild {
            self.rebuild(gpu)?;
        }

        Ok(())
    }

    pub fn draw_with_encoder(
        &mut self,
        gpu: &Gpu,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        world: &mut World,
        assets: &AssetCache,
        external_resources: &ExternalResources,
    ) -> anyhow::Result<()> {
        profile_function!();

        let Some(order) = &self.order else {
            anyhow::bail!("update must be called before draw");
        };

        for &idx in order {
            let node = &mut self.nodes[idx];
            profile_scope!("render_node", node.label());

            node.draw(NodeExecutionContext {
                gpu,
                resources: &self.resources,
                queue,
                encoder,
                assets,
                world,
                external_resources,
            })?;
        }

        Ok(())
    }

    fn invoke_on_resource_modified(&mut self) {
        for &modified in self.resources.modified_resources.iter() {
            self.resource_to_nodes
                .get(&modified)
                .iter()
                .flat_map(|v| *v)
                .for_each(|&idx| {
                    self.nodes[idx].on_resource_changed(modified);
                })
        }
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
    order: Vec<NodeId>,
    dependency_levels: SecondaryMap<NodeId, u32>,
}

fn topo_sort(
    nodes: &SlotMap<NodeId, Box<dyn Node>>,
    edges: &HashMap<NodeId, Vec<NodeId>>,
) -> TopoResult {
    profile_function!();
    let mut result = Vec::new();

    let mut visited = nodes.keys().map(|v| (v, VisitedState::None)).collect();
    let mut dependency_levels = nodes.keys().map(|v| (v, 0)).collect();

    #[derive(Clone, Copy)]
    enum VisitedState {
        None,
        Pending,
        Visited,
    }

    fn visit(
        edges: &HashMap<NodeId, Vec<NodeId>>,
        visited: &mut SecondaryMap<NodeId, VisitedState>,
        result: &mut Vec<NodeId>,
        dependency_levels: &mut SecondaryMap<NodeId, u32>,
        node: NodeId,
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

    for node in nodes.keys() {
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
    use std::sync::Arc;

    use ivy_wgpu_types::{Gpu, TypedBuffer};
    use wgpu::{
        BufferUsages, Extent3d, ImageCopyBuffer, ImageCopyTexture, ImageDataLayout,
        TextureDimension, TextureFormat, TextureUsages,
    };

    use crate::{
        rendergraph::{
            BufferDesc, BufferHandle, Dependency, ManagedTextureDesc, Node, NodeExecutionContext,
            RenderGraph, RenderGraphResources, TextureHandle,
        },
        shader_library::ShaderLibrary,
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

            fn on_resource_changed(&mut self, _resource: super::ResourceHandle) {}
        }

        let mut resources = RenderGraphResources::new(Arc::new(ShaderLibrary::new()));
        let mut render_graph = RenderGraph::new(resources);

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

        let mut encoder = gpu.device.create_command_encoder(&Default::default());

        render_graph
            .draw_with_encoder(
                &gpu,
                &gpu.queue,
                &mut encoder,
                &mut Default::default(),
                &Default::default(),
                &Default::default(),
            )
            .unwrap();

        gpu.queue.submit([encoder.finish()]);

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
