mod resources;
use flax::World;
use ivy_assets::AssetCache;
pub use resources::*;

use std::collections::HashMap;

use itertools::Itertools;
use ivy_wgpu_types::Gpu;
use wgpu::{BufferUsages, CommandEncoder, Queue, TextureUsages};

pub struct RenderGraph {
    nodes: Vec<Box<dyn Node>>,
    order: Option<Vec<usize>>,
    pub resources: Resources,
    expected_lifetimes: HashMap<ResourceHandle, Lifetime>,
}

pub struct NodeExecutionContext<'a> {
    pub gpu: &'a Gpu,
    pub resources: &'a Resources,
    pub queue: &'a Queue,
    pub encoder: &'a mut CommandEncoder,
    pub assets: &'a AssetCache,
    pub world: &'a mut World,
}

pub trait Node: 'static {
    fn label(&self) -> &str;
    fn draw(&mut self, ctx: NodeExecutionContext) -> anyhow::Result<()>;
    fn read_dependencies(&self) -> Vec<Dependency>;
    fn write_dependencies(&self) -> Vec<Dependency>;
    fn finish(&mut self) -> anyhow::Result<()> {
        Ok(())
    }
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
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

impl RenderGraph {
    pub fn new() -> Self {
        Self {
            nodes: Vec::new(),
            order: None,
            resources: Default::default(),
            expected_lifetimes: Default::default(),
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

    fn build(&mut self) {
        let writes: HashMap<_, _> = self
            .nodes
            .iter()
            .enumerate()
            .flat_map(|(idx, v)| {
                v.write_dependencies()
                    .into_iter()
                    .map(move |d| (d.as_handle(), idx))
            })
            .collect();

        let reads = self
            .nodes
            .iter()
            .enumerate()
            .flat_map(|(idx, v)| {
                v.read_dependencies()
                    .into_iter()
                    .map(move |v| (v.as_handle(), idx))
            })
            .into_group_map();

        let dependencies = self
            .nodes
            .iter()
            .enumerate()
            .flat_map(|(node_idx, node)| {
                let writes = &writes;
                node.read_dependencies().into_iter().map(move |v| {
                    let write_idx = *writes.get(&v.as_handle()).unwrap();
                    (node_idx, write_idx)
                })
            })
            .into_group_map();

        let TopoResult {
            order,
            dependency_levels,
        } = topo_sort(&self.nodes, &dependencies);

        self.expected_lifetimes.clear();
        tracing::info!(?writes, "writes");
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
                .insert(resource, Lifetime::new(open, close));
        }

        self.order = Some(order);
    }

    pub fn execute(
        &mut self,
        gpu: &Gpu,
        queue: &Queue,
        world: &mut World,
        assets: &AssetCache,
    ) -> anyhow::Result<()> {
        if self.order.is_none() {
            self.build();
            self.allocate_resources(gpu);
        }

        let order = self.order.as_ref().unwrap();

        let mut encoder = gpu.device.create_command_encoder(&Default::default());

        for &idx in order {
            let node = &mut self.nodes[idx];
            tracing::info!(?idx, label = node.label(), "executing node");
            node.draw(NodeExecutionContext {
                gpu,
                resources: &self.resources,
                queue,
                encoder: &mut encoder,
                assets,
                world,
            })?;
        }

        queue.submit([encoder.finish()]);

        for &idx in order {
            let node = &mut self.nodes[idx];
            node.finish()?;
        }

        Ok(())
    }
}

impl Default for RenderGraph {
    fn default() -> Self {
        Self::new()
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
        BufferDesc, BufferHandle, Dependency, Node, NodeExecutionContext, RenderGraph, TextureDesc,
        TextureHandle,
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
                let texture = ctx.resources.get_texture_data(self.texture);
                let buffer = ctx.resources.get_buffer_data(self.buffer);

                // ctx.encoder.copy_buffer_to_texture(
                //     ImageCopyBuffer {
                //         buffer,
                //         layout: ImageDataLayout {
                //             offset: 0,
                //             bytes_per_row: Some(256),
                //             rows_per_image: Some(256),
                //         },
                //     },
                //     ImageCopyTexture {
                //         texture,
                //         mip_level: 0,
                //         origin: Default::default(),
                //         aspect: wgpu::TextureAspect::All,
                //     },
                //     Extent3d {
                //         width: 256,
                //         height: 256,
                //         depth_or_array_layers: 1,
                //     },
                // );

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
        }

        let mut render_graph = RenderGraph::new();

        let extent = Extent3d {
            width: 256,
            height: 256,
            depth_or_array_layers: 1,
        };

        let texture = render_graph.resources.insert_texture(TextureDesc {
            label: "src_texture".into(),
            extent,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8Uint,
            mip_level_count: 1,
            sample_count: 1,
        });

        let texture2 = render_graph.resources.insert_texture(TextureDesc {
            label: "texture_2".into(),
            extent,
            dimension: TextureDimension::D2,
            format: TextureFormat::R8Uint,
            mip_level_count: 1,
            sample_count: 1,
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
            .execute(
                &gpu,
                &gpu.queue,
                &mut Default::default(),
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
