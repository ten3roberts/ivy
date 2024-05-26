use std::iter::repeat;

use glam::{vec2, vec3, Vec2, Vec3, Vec4};
use itertools::izip;
use ivy_assets::{Asset, AssetCache};
use wgpu::{
    util::DeviceExt, vertex_attr_array, Buffer, RenderPass, VertexAttribute, VertexBufferLayout,
};

use crate::mesh::{MeshData, MeshDesc};

use super::{material::Material, Gpu};

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Copy, Debug, Clone)]
pub struct Vertex {
    pos: Vec3,
    tex_coord: Vec2,
    normal: Vec3,
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
        }
    }
}
impl VertexDesc for Vertex {
    fn layout() -> VertexBufferLayout<'static> {
        static ATTRIBUTES: &[VertexAttribute] =
            &vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x3];

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

#[derive(Debug)]
pub struct Mesh {
    vertex_count: u32,
    index_count: u32,
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    primitives: Option<Vec<Primitive>>,
}

impl Mesh {
    pub fn new(
        gpu: &Gpu,
        vertices: &[Vertex],
        indices: &[u32],
        primitives: Option<Vec<Primitive>>,
    ) -> Self {
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
            primitives,
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

    pub(crate) fn from_gltf(
        gpu: &Gpu,
        assets: &AssetCache,
        mesh: gltf::Mesh,
        buffer_data: &[gltf::buffer::Data],
        materials: &[Asset<Material>],
    ) -> Self {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let mut primitives = Vec::new();

        for p in mesh.primitives() {
            let reader = p.reader(|buffer| Some(&buffer_data[buffer.index()]));

            let first_index = indices.len() as u32;
            let offset = vertices.len() as u32;
            indices.extend(
                reader
                    .read_indices()
                    .into_iter()
                    .flat_map(|val| val.into_u32())
                    .map(|val| val + offset),
            );

            let index_count = indices.len() as u32 - first_index;

            let pos = reader
                .read_positions()
                .into_iter()
                .flatten()
                .map(Vec3::from);

            let normals = reader.read_normals().into_iter().flatten().map(Vec3::from);

            let texcoord = reader
                .read_tex_coords(0)
                .into_iter()
                .flat_map(|val| val.into_f32())
                .map(Vec2::from);

            vertices.extend(
                izip!(pos, normals, texcoord, repeat(Vec3::ZERO))
                    .map(|(pos, normal, textcoord, tangent)| Vertex::new(pos, textcoord, normal)),
            );

            // Keep track of which materials map to which part of the index buffer
            if let Some(material) = materials.get(p.material().index().unwrap_or(0)) {
                primitives.push(Primitive {
                    first_index,
                    index_count,
                    material: material.clone(),
                });
            }
        }

        Self::new(gpu, &vertices, &indices, Some(primitives))
    }
}
