use std::sync::Arc;

use ash::vk::{DescriptorSet, ShaderStageFlags};
use ivy_vulkan::{
    descriptors::{DescriptorAllocator, DescriptorBuilder, DescriptorLayoutCache},
    Sampler, Texture, VulkanContext,
};

use crate::Error;

/// A material contains shader properties such as albedo and other parameters. A material does not
/// define the graphics pipeline nor shaders as that is per pass dependent.
pub struct Material {
    set: DescriptorSet,
    albedo: Arc<Texture>,
    sampler: Arc<Sampler>,
}

impl Material {
    /// Creates a new material with albedo using the provided sampler
    pub fn new(
        context: Arc<VulkanContext>,
        descriptor_layout_cache: &mut DescriptorLayoutCache,
        descriptor_allocator: &mut DescriptorAllocator,
        albedo: Arc<Texture>,
        sampler: Arc<Sampler>,
    ) -> Result<Self, Error> {
        let mut set = Default::default();

        DescriptorBuilder::new()
            .bind_combined_image_sampler(0, ShaderStageFlags::FRAGMENT, &albedo, &sampler)
            .build(
                context.device(),
                descriptor_layout_cache,
                descriptor_allocator,
                &mut set,
            )?;

        Ok(Self {
            set,
            albedo,
            sampler,
        })
    }

    /// Get a reference to the material's descriptor set.
    pub fn set(&self) -> DescriptorSet {
        self.set
    }

    pub fn albedo(&self) -> &Arc<Texture> {
        &self.albedo
    }

    pub fn sampler(&self) -> &Arc<Sampler> {
        &self.sampler
    }
}
