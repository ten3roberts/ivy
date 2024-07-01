use glam::{Vec2, Vec3, Vec4};
use ivy_assets::Asset;
use wgpu::{
    util::DeviceExt, vertex_attr_array, Buffer, RenderPass, VertexAttribute, VertexBufferLayout,
};

use crate::material::Material;

use super::Gpu;

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Copy, Debug, Clone)]
pub struct Vertex {
    pub pos: Vec3,
    pub tex_coord: Vec2,
    pub normal: Vec3,
    pub tangent: Vec4,
}

pub trait VertexDesc {
    fn layout() -> VertexBufferLayout<'static>;
}

impl Vertex {
    pub const fn new(pos: Vec3, tex_coord: Vec2, normal: Vec3) -> Self {
        Self {
            pos,
            tex_coord,
            normal,
            tangent: Vec4::ZERO,
        }
    }
}
impl VertexDesc for Vertex {
    fn layout() -> VertexBufferLayout<'static> {
        static ATTRIBUTES: &[VertexAttribute] =
            &vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x3, 3 => Float32x4];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: ATTRIBUTES,
        }
    }
}

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Copy, Debug, Clone)]
pub struct Vertex2d {
    pos: Vec2,
    tex_coord: Vec2,
}

impl Vertex2d {
    pub const fn new(pos: Vec2, tex_coord: Vec2) -> Self {
        Self { pos, tex_coord }
    }
}
impl VertexDesc for Vertex2d {
    fn layout() -> VertexBufferLayout<'static> {
        static ATTRIBUTES: &[VertexAttribute] = &vertex_attr_array![0 => Float32x3, 1 => Float32x2];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: ATTRIBUTES,
        }
    }
}

#[derive(Debug)]
pub struct Primitive {
    pub first_index: u32,
    pub index_count: u32,
    pub material: Asset<Material>,
}

/// Flat mesh of vertices and indices
///
/// For Gltf, contains the vertices and indices of *all* primitives.
#[derive(Debug)]
pub struct Mesh {
    vertex_count: u32,
    index_count: u32,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
}

impl Mesh {
    pub fn new(gpu: &Gpu, vertices: &[Vertex], indices: &[u32]) -> Self {
        let vertex_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Vertex Buffer"),
                contents: bytemuck::cast_slice(vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });

        let index_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Index Buffer"),
                contents: bytemuck::cast_slice(indices),
                usage: wgpu::BufferUsages::INDEX,
            });

        Self {
            vertex_count: vertices.len() as u32,
            index_count: indices.len() as u32,
            vertex_buffer,
            index_buffer,
        }
    }

    pub fn bind<'a>(&'a self, render_pass: &mut RenderPass<'a>) {
        render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
    }

    pub fn index_count(&self) -> u32 {
        self.index_count
    }

    pub fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    pub fn vertex_buffer(&self) -> &Buffer {
        &self.vertex_buffer
    }

    pub fn index_buffer(&self) -> &Buffer {
        &self.index_buffer
    }
}
