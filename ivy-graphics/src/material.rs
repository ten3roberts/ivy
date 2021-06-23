use ash::vk::{DescriptorSet, DescriptorSetLayout, ShaderStageFlags};
use ivy_resources::{Handle, ResourceCache};
use ivy_vulkan::{
    descriptors::{DescriptorAllocator, DescriptorBuilder, DescriptorLayoutCache},
    Sampler, Texture, VulkanContext,
};

use crate::Error;

/// A material contains shader properties such as albedo and other parameters. A material does not
/// define the graphics pipeline nor shaders as that is per pass dependent.
pub struct Material {
    layout: DescriptorSetLayout,
    set: DescriptorSet,
    albedo: Handle<Texture>,
    sampler: Handle<Sampler>,
}

impl Material {
    /// Creates a new material with albedo using the provided sampler
    pub fn new(
        context: &VulkanContext,
        descriptor_layout_cache: &mut DescriptorLayoutCache,
        descriptor_allocator: &mut DescriptorAllocator,
        textures: &ResourceCache<Texture>,
        samplers: &ResourceCache<Sampler>,
        albedo: Handle<Texture>,
        sampler: Handle<Sampler>,
    ) -> Result<Self, Error> {
        let (set, layout) = DescriptorBuilder::new()
            .bind_combined_image_sampler(
                0,
                ShaderStageFlags::FRAGMENT,
                textures.get(albedo)?,
                samplers.get(sampler)?,
            )
            .build_one(
                context.device(),
                descriptor_layout_cache,
                descriptor_allocator,
            )?;

        Ok(Self {
            layout,
            set,
            albedo,
            sampler,
        })
    }

    /// Get a reference to the material's descriptor set.
    pub fn layout(&self) -> DescriptorSetLayout {
        self.layout
    }

    /// Get a reference to the material's descriptor set.
    pub fn set(&self) -> DescriptorSet {
        self.set
    }

    pub fn albedo(&self) -> Handle<Texture> {
        self.albedo
    }

    pub fn sampler(&self) -> Handle<Sampler> {
        self.sampler
    }
}
