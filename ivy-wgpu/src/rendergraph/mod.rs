mod resources;
pub use resources::*;

use std::collections::HashMap;

use itertools::Itertools;
use ivy_wgpu_types::Gpu;
use wgpu::{BufferUsages, CommandEncoder, Queue, TextureUsages};

pub struct RenderGraph {
    nodes: Vec<Box<dyn Node>>,
    order: Option<Vec<usize>>,
    resources: Resources,
}

pub struct NodeExecutionContext<'a> {
    pub resources: &'a Resources,
    pub queue: &'a Queue,
    pub encoder: &'a mut CommandEncoder,
}

pub trait Node: 'static {
    fn label(&self) -> &str;
    fn draw(&self, ctx: NodeExecutionContext) -> anyhow::Result<()>;
    fn read_dependencies(&self) -> &[Dependency];
    fn write_dependencies(&self) -> &[Dependency];
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
        }
    }

    pub fn add_node(&mut self, node: impl Node) {
        self.nodes.push(Box::new(node));
    }

    pub fn allocate_resources(&mut self, gpu: &Gpu) {
        self.resources.allocate_textures(&self.nodes, gpu);
        self.resources.allocate_buffers(&self.nodes, gpu);
    }

    fn build(&mut self) {
        let writes: HashMap<_, _> = self
            .nodes
            .iter()
            .enumerate()
            .flat_map(|v| {
                v.1.write_dependencies()
                    .iter()
                    .map(move |d| (d.as_handle(), v.0))
            })
            .collect();

        let dependencies = self
            .nodes
            .iter()
            .enumerate()
            .flat_map(|(node_idx, node)| {
                let writes = &writes;
                node.read_dependencies().iter().map(move |v| {
                    let write_idx = *writes.get(&v.as_handle()).unwrap();
                    (node_idx, write_idx)
                })
            })
            .into_group_map();

        let order = topo_sort(&self.nodes, &dependencies);

        self.order = Some(order);
    }

    pub fn execute(&mut self, gpu: &Gpu, queue: &Queue) -> anyhow::Result<()> {
        if self.order.is_none() {
            self.build()
        }

        let order = self.order.as_ref().unwrap();

        let mut encoder = gpu.device.create_command_encoder(&Default::default());

        for &idx in order {
            let node = &mut self.nodes[idx];
            tracing::info!(?idx, label = node.label(), "executing node");
            node.draw(NodeExecutionContext {
                resources: &self.resources,
                queue,
                encoder: &mut encoder,
            })?;
        }

        queue.submit([encoder.finish()]);

        Ok(())
    }
}

impl Default for RenderGraph {
    fn default() -> Self {
        Self::new()
    }
}

fn topo_sort(nodes: &[Box<dyn Node>], edges: &HashMap<usize, Vec<usize>>) -> Vec<usize> {
    let mut visited = vec![VisitedState::None; nodes.len()];
    let mut result = Vec::new();

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
        node: usize,
    ) {
        match visited[node] {
            VisitedState::None => {}
            VisitedState::Pending => {
                panic!("cyclic dependency");
            }
            VisitedState::Visited => {
                return;
            }
        }

        visited[node] = VisitedState::Pending;

        for &outgoing in edges.get(&node).into_iter().flatten() {
            visit(edges, visited, result, outgoing)
        }

        visited[node] = VisitedState::Visited;

        result.push(node)
    }

    for node in 0..nodes.len() {
        visit(edges, &mut visited, &mut result, node)
    }

    result
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
            write_dependencies: Vec<Dependency>,
        }

        impl WriteToTexture {
            fn new(buffer: TypedBuffer<u8>, write: TextureHandle) -> Self {
                Self {
                    buffer,
                    write,
                    write_dependencies: vec![Dependency::texture(write, TextureUsages::COPY_DST)],
                }
            }
        }

        impl Node for WriteToTexture {
            fn label(&self) -> &str {
                "WriteToTexture"
            }

            fn draw(&self, ctx: NodeExecutionContext) -> anyhow::Result<()> {
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

            fn read_dependencies(&self) -> &[crate::rendergraph::Dependency] {
                &[]
            }

            fn write_dependencies(&self) -> &[crate::rendergraph::Dependency] {
                &self.write_dependencies
            }
        }

        struct ReadFromTexture {
            read_texture: TextureHandle,
            write_buffer: BufferHandle,
            read_dependencies: Vec<Dependency>,
            write_dependencies: Vec<Dependency>,
        }

        impl ReadFromTexture {
            fn new(read_texture: TextureHandle, write_buffer: BufferHandle) -> Self {
                Self {
                    read_texture,
                    write_buffer,
                    read_dependencies: vec![Dependency::texture(
                        read_texture,
                        TextureUsages::COPY_SRC,
                    )],
                    write_dependencies: vec![Dependency::buffer(
                        write_buffer,
                        BufferUsages::COPY_DST,
                    )],
                }
            }
        }

        impl Node for ReadFromTexture {
            fn label(&self) -> &str {
                "ReadFromTexture"
            }

            fn draw(&self, ctx: NodeExecutionContext) -> anyhow::Result<()> {
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

            fn read_dependencies(&self) -> &[crate::rendergraph::Dependency] {
                &self.read_dependencies
            }

            fn write_dependencies(&self) -> &[crate::rendergraph::Dependency] {
                &self.write_dependencies
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

        render_graph.allocate_resources(&gpu);
        render_graph.execute(&gpu, &gpu.queue).unwrap();

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