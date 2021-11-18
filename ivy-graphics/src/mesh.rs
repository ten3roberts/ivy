use crate::{Material, Result};
use ash::vk;
use gltf::{buffer, Semantic};
use ivy_resources::Handle;
use std::mem::size_of;
use std::sync::Arc;
use std::{iter::repeat, marker::PhantomData};
use ultraviolet::{Vec2, Vec3};

use crate::Error;
use ivy_vulkan as vulkan;
use vulkan::{Buffer, BufferAccess, BufferUsage, VertexDesc, VulkanContext};

#[derive(Debug, Clone, Copy, PartialEq)]
/// A simple vertex type with position, normal and texcoord.
pub struct Vertex {
    position: Vec3,
    normal: Vec3,
    texcoord: Vec2,
}

impl Vertex {
    pub fn new(position: Vec3, normal: Vec3, texcoord: Vec2) -> Self {
        Self {
            position,
            normal,
            texcoord,
        }
    }
}

impl vulkan::VertexDesc for Vertex {
    const BINDING_DESCRIPTIONS: &'static [vk::VertexInputBindingDescription] =
        &[vk::VertexInputBindingDescription {
            binding: 0,
            stride: size_of::<Self>() as u32,
            input_rate: vk::VertexInputRate::VERTEX,
        }];

    const ATTRIBUTE_DESCRIPTIONS: &'static [vk::VertexInputAttributeDescription] = &[
        // vec3 3*4 bytes
        vk::VertexInputAttributeDescription {
            binding: 0,
            location: 0,
            format: vk::Format::R32G32B32_SFLOAT,
            offset: 0,
        },
        // vec3 3*4 bytes
        vk::VertexInputAttributeDescription {
            binding: 0,
            location: 1,
            format: vk::Format::R32G32B32_SFLOAT,
            offset: 12,
        },
        // vec2 2*4 bytes
        vk::VertexInputAttributeDescription {
            binding: 0,
            location: 2,
            format: vk::Format::R32G32_SFLOAT,
            offset: 12 + 12,
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
            Vertex::new(
                Vec3::new(-hw, -hh, 0.0),
                Vec3::unit_x(),
                Vec2::new(0.0, 1.0),
            ),
            Vertex::new(Vec3::new(hw, -hh, 0.0), Vec3::unit_x(), Vec2::new(1.0, 1.0)),
            Vertex::new(Vec3::new(hw, hh, 0.0), Vec3::unit_x(), Vec2::new(1.0, 0.0)),
            Vertex::new(Vec3::new(-hw, hh, 0.0), Vec3::unit_x(), Vec2::new(0.0, 0.0)),
        ];

        let indices: [u32; 6] = [0, 1, 2, 2, 3, 0];
        Self::new(context, &vertices, &indices, vec![])
    }

    /// Creates a mesh from an structure-of-arrays vertex data
    /// Each index refers to the direct index of positions, normals and texcoords
    pub fn from_soa(
        context: Arc<VulkanContext>,
        positions: &[Vec3],
        normals: &[Vec3],
        texcoords: &[Vec2],
        indices: &[u32],
        primitives: Vec<Primitive>,
    ) -> Result<Self> {
        let mut vertices = Vec::with_capacity(positions.len());

        for i in 0..positions.len() {
            vertices.push(Vertex::new(positions[i], normals[i], texcoords[i]));
        }

        Self::new(context, vertices.as_slice(), &indices, primitives)
    }

    /// Loads a mesh from gltf asset. Loads positions, normals, and texture coordinates.
    pub fn from_gltf(
        context: Arc<VulkanContext>,
        mesh: gltf::Mesh,
        buffers: &[buffer::Data],
        materials: &[Handle<Material>],
    ) -> Result<Self> {
        let mut positions = Vec::new();
        let mut normals = Vec::new();
        let mut texcoords = Vec::new();
        let mut indices = Vec::new();

        let mut primitives = Vec::new();

        for primitive in mesh.primitives() {
            let indices_accessor = primitive.indices().ok_or(Error::SparseAccessor)?;
            let indices_view = indices_accessor.view().ok_or(Error::SparseAccessor)?;

            let first_index = indices.len() as u32;
            match indices_accessor.size() {
                2 => load_u16_as_u32(&indices_view, buffers, &mut indices, positions.len() as u32),
                4 => load_u32(&indices_view, buffers, &mut indices, positions.len() as u32),
                _ => unimplemented!(),
            }

            let index_count = indices.len() as u32 - first_index;

            // Keep track of which materials map to which part of the index buffer
            if let Some(material) = materials.get(primitive.material().index().unwrap_or(0)) {
                primitives.push(Primitive {
                    first_index,
                    index_count,
                    material: *material,
                });
            }

            for (semantic, accessor) in primitive.attributes() {
                let view = accessor.view().ok_or(Error::SparseAccessor)?;
                match semantic {
                    Semantic::Positions => load_vec3(&view, buffers, &mut positions),
                    Semantic::Normals => load_vec3(&view, buffers, &mut normals),
                    Semantic::TexCoords(_) => load_vec2(&view, buffers, &mut texcoords),
                    Semantic::Tangents => {}
                    Semantic::Colors(_) => {}
                    Semantic::Joints(_) => {}
                    Semantic::Weights(_) => {}
                };
            }
        }

        // Pad incase these weren't included in geometry
        pad_vec(&mut normals, Vec3::unit_z(), positions.len());
        pad_vec(&mut texcoords, Vec2::zero(), positions.len());

        Self::from_soa(
            context, &positions, &normals, &texcoords, &indices, primitives,
        )
    }
}

// Pads a vector with copies of val to ensure it is atleast `len` elements
fn pad_vec<T: Copy>(vec: &mut Vec<T>, val: T, len: usize) {
    vec.extend(repeat(val).take(len - vec.len()))
}

fn load_u16_as_u32(
    view: &buffer::View,
    buffers: &[buffer::Data],
    indices: &mut Vec<u32>,
    offset: u32,
) {
    let buffer = &buffers[view.buffer().index()];

    let raw_data = &buffer[view.offset()..view.offset() + view.length()];
    indices.extend(
        raw_data
            .chunks_exact(2)
            .map(|val| u16::from_le_bytes([val[0], val[1]]) as u32 + offset),
    )
}

fn load_u32(view: &buffer::View, buffers: &[buffer::Data], indices: &mut Vec<u32>, offset: u32) {
    let buffer = &buffers[view.buffer().index()];

    let raw_data = &buffer[view.offset()..view.offset() + view.length()];
    indices.extend(
        raw_data
            .chunks_exact(4)
            .map(|val| u32::from_le_bytes([val[0], val[1], val[2], val[3]]) + offset),
    )
}

fn load_vec2(view: &buffer::View, buffers: &[buffer::Data], buf: &mut Vec<Vec2>) {
    let buffer = &buffers[view.buffer().index()];

    let raw_data = &buffer[view.offset()..view.offset() + view.length()];
    buf.extend(raw_data.chunks_exact(8).map(|val| {
        Vec2::new(
            f32::from_le_bytes([val[0], val[1], val[2], val[3]]),
            f32::from_le_bytes([val[4], val[5], val[6], val[7]]),
        )
    }))
}

fn load_vec3(view: &buffer::View, buffers: &[buffer::Data], buf: &mut Vec<Vec3>) {
    let buffer = &buffers[view.buffer().index()];

    let raw_data = &buffer[view.offset()..view.offset() + view.length()];
    buf.extend(raw_data.chunks_exact(12).map(|val| {
        Vec3::new(
            f32::from_le_bytes([val[0], val[1], val[2], val[3]]),
            f32::from_le_bytes([val[4], val[5], val[6], val[7]]),
            f32::from_le_bytes([val[8], val[9], val[10], val[11]]),
        )
    }))
}
