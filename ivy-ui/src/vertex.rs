use std::mem;

use ivy_vulkan::{vk, VertexDesc};
use ultraviolet::{Vec2, Vec3};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
/// A simple vertex type with position, normal and texcoord.
pub struct UIVertex {
    position: Vec3,
    texcoord: Vec2,
}

impl UIVertex {
    pub fn new(position: Vec3, texcoord: Vec2) -> Self {
        Self { position, texcoord }
    }
}

impl VertexDesc for UIVertex {
    const BINDING_DESCRIPTIONS: &'static [vk::VertexInputBindingDescription] =
        &[vk::VertexInputBindingDescription {
            binding: 0,
            stride: mem::size_of::<Self>() as u32,
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
        // vec2 2*4 bytes
        vk::VertexInputAttributeDescription {
            binding: 0,
            location: 1,
            format: vk::Format::R32G32_SFLOAT,
            offset: 12,
        },
    ];
}
