use crate::Result;
use ivy_resources::{Handle, ResourceManager};
use ivy_vulkan::{
    descriptors::{DescriptorAllocator, DescriptorBuilder, DescriptorLayoutCache, DescriptorSet},
    vk::ShaderStageFlags,
    Sampler, Texture, VulkanContext,
};

/// A GUI image component containing a texture and associated sampler. The attached widget
/// component will dictate where and how it will be drawn.
pub struct Image {
    set: DescriptorSet,
    texture: Handle<Texture>,
    sampler: Handle<Sampler>,
}

impl Image {
    pub fn new(
        context: &VulkanContext,
        descriptor_layout_cache: &mut DescriptorLayoutCache,
        descriptor_allocator: &mut DescriptorAllocator,
        resources: &ResourceManager,
        texture: Handle<Texture>,
        sampler: Handle<Sampler>,
    ) -> Result<Self> {
        let set = DescriptorBuilder::new()
            .bind_combined_image_sampler(
                0,
                ShaderStageFlags::FRAGMENT,
                resources.get(texture)?.image_view(),
                resources.get(sampler)?.sampler(),
            )
            .build_one(
                context.device(),
                descriptor_layout_cache,
                descriptor_allocator,
            )?
            .0;

        Ok(Self {
            set,
            texture,
            sampler,
        })
    }

    /// Get a reference to the image's texture.
    pub fn texture(&self) -> Handle<Texture> {
        self.texture
    }

    /// Get a reference to the image's sampler.
    pub fn sampler(&self) -> Handle<Sampler> {
        self.sampler
    }

    /// Get a reference to the image's set.
    pub fn set(&self) -> DescriptorSet {
        self.set
    }
}
