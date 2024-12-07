use std::sync::{Arc, Mutex};

use bytemuck::Pod;
use itertools::Itertools;
use wgpu::{BufferUsages, RenderPass};

use super::{
    types::multi_buffer::{MultiBuffer, SubBuffer},
    Gpu,
};
use crate::mesh::Vertex;

pub struct MeshBufferInner {}

type DroppedList<V> = Vec<(SubBuffer<V>, SubBuffer<u32>)>;

pub struct MeshBuffer<V = Vertex> {
    next_id: u64,
    pub vertex_buffers: MultiBuffer<V>,
    pub index_buffers: MultiBuffer<u32>,
    dropped: Arc<Mutex<DroppedList<V>>>,
}

/// Handle to an allocation within a mesh
#[derive(Clone)]
pub struct MeshHandle<V = Vertex> {
    id: u64,
    vb: SubBuffer<V>,
    ib: SubBuffer<u32>,
    index_count: usize,
    on_drop: Arc<Mutex<DroppedList<V>>>,
}

impl<V> std::fmt::Debug for MeshHandle<V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MeshHandle")
            .field("id", &self.id)
            .field("vb", &self.vb)
            .field("ib", &self.ib)
            .finish()
    }
}

impl<V> std::hash::Hash for MeshHandle<V> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<V> Eq for MeshHandle<V> {}

impl<V> PartialEq for MeshHandle<V> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<V> MeshHandle<V> {
    pub fn vb(&self) -> SubBuffer<V> {
        self.vb
    }

    pub fn ib(&self) -> SubBuffer<u32> {
        self.ib
    }

    pub fn index_count(&self) -> usize {
        self.index_count
    }
}

impl<V> Drop for MeshHandle<V> {
    fn drop(&mut self) {
        self.on_drop.lock().unwrap().push((self.vb, self.ib));
    }
}

impl<V: Pod> MeshBuffer<V> {
    pub fn new(gpu: &Gpu, label: impl Into<String>, capacity: usize) -> Self {
        let label = label.into();

        let vertex_buffer = MultiBuffer::new(
            gpu,
            format!("{}::vertex_buffer", label),
            BufferUsages::VERTEX | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            capacity,
        );
        let index_buffer = MultiBuffer::new(
            gpu,
            format!("{}::index_buffer", label),
            BufferUsages::INDEX | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            capacity,
        );

        Self {
            vertex_buffers: vertex_buffer,
            index_buffers: index_buffer,
            dropped: Arc::default(),
            next_id: 0,
        }
    }

    fn reclaim(&mut self) {
        for (vb, ib) in self.dropped.lock().unwrap().drain(..) {
            tracing::debug!(?vb, ?ib, "reclaim");
            self.vertex_buffers.deallocate(vb);
            self.index_buffers.deallocate(ib);
        }
    }

    /// Allocate a mesh in the buffer
    pub fn allocate(
        &mut self,
        gpu: &Gpu,
        vertex_count: usize,
        index_count: usize,
    ) -> MeshHandle<V> {
        self.allocate_exact(gpu, vertex_count, index_count)
    }

    pub fn allocate_exact(
        &mut self,
        gpu: &Gpu,
        vertex_count: usize,
        index_count: usize,
    ) -> MeshHandle<V> {
        self.reclaim();
        tracing::debug!("Allocating {vertex_count} {index_count}");
        let vb = match self.vertex_buffers.allocate(vertex_count) {
            Some(v) => v,
            None => {
                self.vertex_buffers.grow(gpu, vertex_count);
                self.vertex_buffers.allocate(vertex_count).unwrap()
            }
        };

        let ib = match self.index_buffers.allocate(index_count) {
            Some(v) => v,
            None => {
                self.index_buffers.grow(gpu, index_count);
                self.index_buffers.allocate(index_count).unwrap()
            }
        };

        let next_id = self.next_id;
        self.next_id += 1;

        MeshHandle {
            id: next_id,
            vb,
            ib,
            on_drop: self.dropped.clone(),
            index_count,
        }
    }

    pub fn insert(&mut self, gpu: &Gpu, vertices: &[V], indices: &[u32]) -> MeshHandle<V> {
        let mesh = self.allocate(gpu, vertices.len(), indices.len());
        self.write(gpu, &mesh, vertices, indices);
        mesh
    }

    pub fn bind<'a>(&'a self, render_pass: &mut RenderPass<'a>) {
        render_pass.set_vertex_buffer(0, self.vertex_buffers.slice(..));
        render_pass.set_index_buffer(self.index_buffers.slice(..), wgpu::IndexFormat::Uint32);
    }

    pub fn write(&mut self, gpu: &Gpu, handle: &MeshHandle<V>, vertices: &[V], indices: &[u32]) {
        self.vertex_buffers.write(&gpu.queue, &handle.vb, vertices);
        self.index_buffers.write(
            &gpu.queue,
            &handle.ib,
            &indices
                .iter()
                .map(|v| v + handle.vb.offset() as u32)
                .collect_vec(),
        );
    }
}
