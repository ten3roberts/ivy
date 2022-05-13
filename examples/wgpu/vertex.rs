use bytemuck::{Pod, Zeroable};
use glam::{Vec2, Vec3};
use std::mem::size_of;
use wgpu::{vertex_attr_array, VertexAttribute, VertexBufferLayout};

#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
#[repr(C)]
pub struct Vertex {
    pub pos: Vec3,
    pub uv: Vec2,
}

impl Vertex {
    fn new(pos: Vec3, uv: Vec2) -> Self {
        Self { pos, uv }
    }

    const ATTRIBUTES: [VertexAttribute; 2] = vertex_attr_array![0 => Float32x3, 1 => Float32x2];

    pub const fn layout() -> VertexBufferLayout<'static> {
        VertexBufferLayout {
            array_stride: size_of::<Self>() as _,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &Self::ATTRIBUTES,
        }
    }
}
