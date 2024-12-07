use glam::{UVec4, Vec2, Vec3, Vec4};
use itertools::{izip, Itertools};
use ivy_assets::Asset;
use ivy_graphics::mesh::{
    MeshData, JOINT_INDEX_ATTRIBUTE, NORMAL_ATTRIBUTE, POSITION_ATTRIBUTE, TANGENT_ATTRIBUTE,
    TEX_COORD_ATTRIBUTE, WEIGHT_ATTRIBUTE,
};
use wgpu::{
    util::DeviceExt, vertex_attr_array, Buffer, RenderPass, VertexAttribute, VertexBufferLayout,
};

use super::Gpu;
use crate::material::PbrMaterial;

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

    pub(crate) fn compose_from_mesh(mesh: &MeshData) -> Vec<Self> {
        let positions = mesh
            .get_attribute(POSITION_ATTRIBUTE)
            .expect("Missing position attribute")
            .as_vec3()
            .unwrap();

        let tex_coords = mesh
            .get_attribute(TEX_COORD_ATTRIBUTE)
            .expect("missing tex_coord attribute")
            .as_vec2()
            .unwrap();

        let normals = mesh
            .get_attribute(NORMAL_ATTRIBUTE)
            .expect("missing normal attribute")
            .as_vec3()
            .unwrap();

        let tangents = mesh
            .get_attribute(TANGENT_ATTRIBUTE)
            .expect("missing tangent attribute")
            .as_vec4()
            .unwrap();

        izip!(positions, tex_coords, normals, tangents)
            .map(|(&pos, &tex_coord, &normal, &tangent)| Self {
                pos,
                tex_coord,
                normal,
                tangent,
            })
            .collect_vec()
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
pub struct SkinnedVertex {
    pub pos: Vec3,
    pub tex_coord: Vec2,
    pub normal: Vec3,
    pub tangent: Vec4,
    pub joints: UVec4,
    pub weights: Vec4,
}

impl SkinnedVertex {
    pub(crate) fn compose_from_mesh(mesh: &MeshData) -> Vec<Self> {
        let positions = mesh
            .get_attribute(POSITION_ATTRIBUTE)
            .expect("Missing position attribute")
            .as_vec3()
            .unwrap();

        let tex_coords = mesh
            .get_attribute(TEX_COORD_ATTRIBUTE)
            .expect("missing tex_coord attribute")
            .as_vec2()
            .unwrap();

        let normals = mesh
            .get_attribute(NORMAL_ATTRIBUTE)
            .expect("missing normal attribute")
            .as_vec3()
            .unwrap();

        let tangents = mesh
            .get_attribute(TANGENT_ATTRIBUTE)
            .expect("missing tangent attribute")
            .as_vec4()
            .unwrap();

        let joints = mesh
            .get_attribute(JOINT_INDEX_ATTRIBUTE)
            .expect("missing joint attribute")
            .as_u16_vec4()
            .unwrap();

        let weights = mesh
            .get_attribute(WEIGHT_ATTRIBUTE)
            .expect("missing weight attribute")
            .as_vec4()
            .unwrap();

        izip!(positions, tex_coords, normals, tangents, joints, weights)
            .map(
                |(&pos, &tex_coord, &normal, &tangent, &joints, &weights)| Self {
                    pos,
                    tex_coord,
                    normal,
                    tangent,
                    joints: joints.into(),
                    weights,
                },
            )
            .collect_vec()
    }
}

impl VertexDesc for SkinnedVertex {
    fn layout() -> VertexBufferLayout<'static> {
        static ATTRIBUTES: &[VertexAttribute] = &vertex_attr_array![0 => Float32x3, 1 => Float32x2, 2 => Float32x3, 3 => Float32x4, 4 => Uint32x4, 5 => Float32x4];

        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Self>() as wgpu::BufferAddress,
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
    pub material: Asset<PbrMaterial>,
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
