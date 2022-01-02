use crate::{Error, Material, Result};
use ash::vk::{
    self, VertexInputAttributeDescription, VertexInputBindingDescription, VertexInputRate,
};
use glam::{IVec4, Vec2, Vec3, Vec4};
use gltf::buffer;
use itertools::izip;
use ivy_resources::Handle;
use std::marker::PhantomData;
use std::mem::size_of;
use std::sync::Arc;

use ivy_vulkan as vulkan;
use vulkan::{Buffer, BufferAccess, BufferUsage, VertexDesc, VulkanContext};

/// A simple vertex type with position, normal and texcoord.
#[records::record]
#[repr(C)]
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    position: Vec3,
    normal: Vec3,
    texcoord: Vec2,
}

/// A skinned vertex type with position, normal, texcoord and skinning
/// information.
#[records::record]
#[repr(C)]
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct SkinnedVertex {
    position: Vec3,
    normal: Vec3,
    texcoord: Vec2,
    /// Joint indices
    joints: IVec4,
    /// Corresponding weight
    weights: Vec4,
}

impl vulkan::VertexDesc for Vertex {
    const BINDING_DESCRIPTIONS: &'static [VertexInputBindingDescription] =
        &[VertexInputBindingDescription {
            binding: 0,
            stride: size_of::<Self>() as u32,
            input_rate: VertexInputRate::VERTEX,
        }];

    const ATTRIBUTE_DESCRIPTIONS: &'static [VertexInputAttributeDescription] = &[
        // vec3 3*4 bytes
        VertexInputAttributeDescription {
            binding: 0,
            location: 0,
            format: vk::Format::R32G32B32_SFLOAT,
            offset: 0,
        },
        // vec3 3*4 bytes
        VertexInputAttributeDescription {
            binding: 0,
            location: 1,
            format: vk::Format::R32G32B32_SFLOAT,
            offset: 12,
        },
        // vec2 2*4 bytes
        VertexInputAttributeDescription {
            binding: 0,
            location: 2,
            format: vk::Format::R32G32_SFLOAT,
            offset: 12 + 12,
        },
    ];
}

impl vulkan::VertexDesc for SkinnedVertex {
    const BINDING_DESCRIPTIONS: &'static [VertexInputBindingDescription] =
        &[VertexInputBindingDescription {
            binding: 0,
            stride: size_of::<Self>() as u32,
            input_rate: VertexInputRate::VERTEX,
        }];

    const ATTRIBUTE_DESCRIPTIONS: &'static [VertexInputAttributeDescription] = &[
        // vec3 3*4 bytes
        VertexInputAttributeDescription {
            binding: 0,
            location: 0,
            format: vk::Format::R32G32B32_SFLOAT,
            offset: 0,
        },
        // vec3 3*4 bytes
        VertexInputAttributeDescription {
            binding: 0,
            location: 1,
            format: vk::Format::R32G32B32_SFLOAT,
            offset: 12,
        },
        // vec2 2*4 bytes
        VertexInputAttributeDescription {
            binding: 0,
            location: 2,
            format: vk::Format::R32G32_SFLOAT,
            offset: 12 + 12,
        },
        VertexInputAttributeDescription {
            binding: 0,
            location: 3,
            format: vk::Format::R32G32B32A32_SINT,
            offset: 12 + 12 + 8,
        },
        VertexInputAttributeDescription {
            binding: 0,
            location: 4,
            format: vk::Format::R32G32B32A32_SFLOAT,
            offset: 12 + 12 + 8 + 16,
        },
    ];
}

/// Represents a part of the mesh with a distincs material.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Primitive {
    pub first_index: u32,
    pub index_count: u32,
    pub material: Handle<Material>,
}

impl Primitive {
    /// Get a reference to the primitive's first index.
    pub fn first_index(&self) -> u32 {
        self.first_index
    }

    /// Get a reference to the primitive's index count.
    pub fn index_count(&self) -> u32 {
        self.index_count
    }

    /// Get a reference to the primitive's material index.
    pub fn material(&self) -> Handle<Material> {
        self.material
    }
}

pub type SkinnedMesh = Mesh<SkinnedVertex>;

/// Represents a vertex and index buffer of `mesh::Vertex` mesh.
pub struct Mesh<V = Vertex> {
    vertex_buffer: Buffer,
    index_buffer: Buffer,
    vertex_count: u32,
    primitives: Vec<Primitive>,
    index_count: u32,
    marker: PhantomData<V>,
}

impl<V: VertexDesc> Mesh<V> {
    /// Creates a new mesh from provided vertices and indices.
    pub fn new(
        context: Arc<VulkanContext>,
        vertices: &[V],
        indices: &[u32],
        primitives: Vec<Primitive>,
    ) -> Result<Self> {
        if vertices.is_empty() {
            return Err(Error::EmptyMesh);
        }

        let vertex_buffer = Buffer::new(
            context.clone(),
            BufferUsage::VERTEX_BUFFER,
            BufferAccess::Staged,
            vertices,
        )?;

        let index_buffer = Buffer::new(
            context,
            BufferUsage::INDEX_BUFFER,
            BufferAccess::Staged,
            indices,
        )?;

        Ok(Self {
            vertex_buffer,
            index_buffer,
            vertex_count: vertices.len() as u32,
            index_count: indices.len() as u32,
            marker: PhantomData,
            primitives,
        })
    }

    /// Creates a new mesh from provided vertices and indices.
    pub fn new_uninit(
        context: Arc<VulkanContext>,
        vertex_count: u32,
        index_count: u32,
        primitives: Vec<Primitive>,
    ) -> Result<Self> {
        let vertex_buffer = Buffer::new_uninit::<V>(
            context.clone(),
            BufferUsage::VERTEX_BUFFER,
            BufferAccess::Staged,
            vertex_count as u64,
        )?;

        let index_buffer = Buffer::new_uninit::<u32>(
            context,
            BufferUsage::INDEX_BUFFER,
            BufferAccess::Staged,
            index_count as u64,
        )?;

        Ok(Self {
            vertex_buffer,
            index_buffer,
            vertex_count,
            index_count,
            marker: PhantomData,
            primitives,
        })
    }
    // Returns the internal vertex buffer
    pub fn vertex_buffer(&self) -> &Buffer {
        &self.vertex_buffer
    }

    // Returns the internal index buffer
    pub fn index_buffer(&self) -> &Buffer {
        &self.index_buffer
    }

    // Returns the number of vertices
    pub fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    // Returns the number of indices
    pub fn index_count(&self) -> u32 {
        self.index_count
    }

    /// Get a mutable reference to the mesh's index buffer.
    pub fn index_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.index_buffer
    }

    /// Get a mutable reference to the mesh's vertex buffer.
    pub fn vertex_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.vertex_buffer
    }

    /// Get a reference to the mesh's primitives.
    pub fn primitives(&self) -> &[Primitive] {
        self.primitives.as_slice()
    }
}

impl Mesh<Vertex> {
    /// Creates a new square or rectangle mesh.
    pub fn new_square(context: Arc<VulkanContext>, width: f32, height: f32) -> Result<Self> {
        let hw = width / 2.0;
        let hh = height / 2.0;

        // Simple quad
        let vertices = [
            Vertex::new(Vec3::new(-hw, -hh, 0.0), Vec3::X, Vec2::new(0.0, 1.0)),
            Vertex::new(Vec3::new(hw, -hh, 0.0), Vec3::X, Vec2::new(1.0, 1.0)),
            Vertex::new(Vec3::new(hw, hh, 0.0), Vec3::X, Vec2::new(1.0, 0.0)),
            Vertex::new(Vec3::new(-hw, hh, 0.0), Vec3::X, Vec2::new(0.0, 0.0)),
        ];

        let indices: [u32; 6] = [0, 1, 2, 2, 3, 0];
        Self::new(context, &vertices, &indices, vec![])
    }

    /// Loads a mesh from gltf asset. Loads positions, normals, and texture coordinates.
    pub fn from_gltf(
        context: Arc<VulkanContext>,
        mesh: gltf::Mesh,
        buffers: &[buffer::Data],
        materials: &[Handle<Material>],
    ) -> Result<Self> {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let mut primitives = Vec::new();

        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
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

            let norm = reader.read_normals().into_iter().flatten().map(Vec3::from);

            let texcoord = reader
                .read_tex_coords(0)
                .into_iter()
                .flat_map(|val| val.into_f32())
                .map(Vec2::from);

            vertices.extend(izip!(pos, norm, texcoord).map(Vertex::from));

            // Keep track of which materials map to which part of the index buffer
            if let Some(material) = materials.get(primitive.material().index().unwrap_or(0)) {
                primitives.push(Primitive {
                    first_index,
                    index_count,
                    material: *material,
                });
            }
        }

        Self::new(context, &vertices, &indices, primitives)
    }
}

impl Mesh<SkinnedVertex> {
    /// Loads a mesh from gltf asset. Loads positions, normals, and texture coordinates.
    pub fn from_gltf_skinned(
        context: Arc<VulkanContext>,
        mesh: gltf::Mesh,
        buffers: &[buffer::Data],
        materials: &[Handle<Material>],
    ) -> Result<Self> {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        let mut primitives = Vec::new();

        for primitive in mesh.primitives() {
            let reader = primitive.reader(|buffer| Some(&buffers[buffer.index()]));
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

            let norm = reader.read_normals().into_iter().flatten().map(Vec3::from);

            let texcoord = reader
                .read_tex_coords(0)
                .into_iter()
                .flat_map(|val| val.into_f32())
                .map(Vec2::from);

            let weights = reader
                .read_weights(0)
                .into_iter()
                .flat_map(|val| val.into_f32())
                .map(Vec4::from);

            let joints = reader
                .read_joints(0)
                .into_iter()
                .flat_map(|val| val.into_u16())
                .map(|val| IVec4::new(val[0] as i32, val[1] as i32, val[2] as i32, val[3] as i32));

            vertices.extend(izip!(pos, norm, texcoord, joints, weights).map(SkinnedVertex::from));

            // Keep track of which materials map to which part of the index buffer
            if let Some(material) = materials.get(primitive.material().index().unwrap_or(0)) {
                primitives.push(Primitive {
                    first_index,
                    index_count,
                    material: *material,
                });
            }
        }

        Self::new(context, &vertices, &indices, primitives)
    }
}
