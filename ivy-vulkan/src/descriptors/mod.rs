use crate::{Result, Sampler, Texture};
use ash::vk;
use ash::Device;
use std::sync::Arc;

mod allocator;
mod builder;
mod layout;
mod traits;

pub use allocator::*;
pub use builder::*;
pub use layout::*;
pub use traits::*;
pub use vk::DescriptorSet;

/// Highest binding allowed for descriptor set. This is deliberately set low as the bindings should
/// be kept as compact as possible.
pub const MAX_BINDINGS: usize = 8;
pub use vk::DescriptorSetLayoutBinding as DescriptorSetBinding;

pub struct DescriptorPool {
    device: Arc<Device>,
    descriptor_pool: vk::DescriptorPool,
}

impl DescriptorPool {
    pub fn new(
        device: Arc<Device>,
        max_sets: u32,
        uniformbuffer_count: u32,
        texture_count: u32,
        input_count: u32,
    ) -> Result<Self> {
        let pool_sizes = [
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::UNIFORM_BUFFER,
                descriptor_count: uniformbuffer_count,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
                descriptor_count: texture_count,
            },
            vk::DescriptorPoolSize {
                ty: vk::DescriptorType::INPUT_ATTACHMENT,
                descriptor_count: input_count,
            },
        ];

        let create_info = vk::DescriptorPoolCreateInfo::builder()
            .pool_sizes(&pool_sizes)
            .max_sets(max_sets);

        let descriptor_pool = unsafe { device.create_descriptor_pool(&create_info, None)? };

        Ok(Self {
            device,
            descriptor_pool,
        })
    }

    /// Allocates descriptor sets from pool
    /// Allocates one descriptor set for each element in `layouts`
    pub fn allocate(&self, layouts: &[vk::DescriptorSetLayout]) -> Result<Vec<vk::DescriptorSet>> {
        let alloc_info = vk::DescriptorSetAllocateInfo {
            s_type: vk::StructureType::DESCRIPTOR_SET_ALLOCATE_INFO,
            p_next: std::ptr::null(),
            descriptor_pool: self.descriptor_pool,
            descriptor_set_count: layouts.len() as _,
            p_set_layouts: layouts.as_ptr(),
        };

        let descriptor_sets = unsafe { self.device.allocate_descriptor_sets(&alloc_info)? };

        Ok(descriptor_sets)
    }

    /// Resets all descriptors allocated from pool
    /// Frees all allocated descriptor sets
    pub fn reset(&self) -> Result<()> {
        unsafe {
            self.device
                .reset_descriptor_pool(self.descriptor_pool, Default::default())?
        };
        Ok(())
    }
}

impl Drop for DescriptorPool {
    fn drop(&mut self) {
        unsafe {
            self.device
                .destroy_descriptor_pool(self.descriptor_pool, None)
        };
    }
}

pub fn write<B>(
    device: &Device,
    descriptor_set: vk::DescriptorSet,
    buffer: B,
    texture: &Texture,
    sampler: &Sampler,
) where
    B: AsRef<vk::Buffer>,
{
    let buffer_info = vk::DescriptorBufferInfo {
        buffer: *buffer.as_ref(),
        offset: 0,
        range: vk::WHOLE_SIZE,
    };

    let image_info = vk::DescriptorImageInfo {
        sampler: sampler.sampler(),
        image_view: texture.image_view(),
        image_layout: vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL,
    };

    let descriptor_writes = [
        vk::WriteDescriptorSet {
            s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
            p_next: std::ptr::null(),
            dst_set: descriptor_set,
            dst_binding: 0,
            dst_array_element: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::UNIFORM_BUFFER,
            p_image_info: std::ptr::null(),
            p_buffer_info: &buffer_info,
            p_texel_buffer_view: std::ptr::null(),
        },
        vk::WriteDescriptorSet {
            s_type: vk::StructureType::WRITE_DESCRIPTOR_SET,
            p_next: std::ptr::null(),
            dst_set: descriptor_set,
            dst_binding: 1,
            dst_array_element: 0,
            descriptor_count: 1,
            descriptor_type: vk::DescriptorType::COMBINED_IMAGE_SAMPLER,
            p_image_info: &image_info,
            p_buffer_info: std::ptr::null(),
            p_texel_buffer_view: std::ptr::null(),
        },
    ];

    unsafe { device.update_descriptor_sets(&descriptor_writes, &[]) };
}
