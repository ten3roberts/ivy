use std::slice;

use crate::Result;
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{
    descriptors::{DescriptorBuilder, DescriptorSet, IntoSet},
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
        resources: &Resources,
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
            .build(&context)?;

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
}

impl IntoSet for Image {
    fn set(&self, _: usize) -> DescriptorSet {
        self.set
    }

    fn sets(&self) -> &[DescriptorSet] {
        slice::from_ref(&self.set)
    }
}
