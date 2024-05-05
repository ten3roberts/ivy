use std::{borrow::Cow, slice};

use crate::{Error, Result};
use ivy_assets::{Asset, AssetCache, AssetKey};
use ivy_vulkan::{
    context::VulkanContextService,
    descriptors::{DescriptorBuilder, DescriptorSet, IntoSet},
    vk::ShaderStageFlags,
    Sampler, SamplerKey, Texture, TextureFromPath,
};

#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};

/// A GUI image component containing a texture and associated sampler. The attached widget
/// component will dictate where and how it will be drawn.
pub struct Image {
    set: DescriptorSet,
    texture: Asset<Texture>,
    sampler: Asset<Sampler>,
}

impl Image {
    pub fn new(
        assets: &AssetCache,
        texture: Asset<Texture>,
        sampler: Asset<Sampler>,
    ) -> Result<Self> {
        let context = assets.service::<VulkanContextService>().context();

        let set = DescriptorBuilder::new()
            .bind_combined_image_sampler(
                0,
                ShaderStageFlags::FRAGMENT,
                texture.image_view(),
                sampler.sampler(),
            )
            .build(&context)?;

        Ok(Self {
            set,
            texture,
            sampler,
        })
    }

    /// Get a reference to the image's texture.
    pub fn texture(&self) -> &Asset<Texture> {
        &self.texture
    }

    /// Get a reference to the image's sampler.
    pub fn sampler(&self) -> &Asset<Sampler> {
        &self.sampler
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

#[derive(Hash, Debug, Clone, PartialEq, Eq, Default)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct ImageInfo {
    pub texture: Cow<'static, str>,
    pub sampler: SamplerKey,
}

impl AssetKey<Image> for ImageInfo {
    type Error = Error;

    fn load(&self, assets: &AssetCache) -> Result<Asset<Image>> {
        let texture = assets.load(&TextureFromPath(self.texture.as_ref().into()));
        let sampler = assets.load(&self.sampler);

        Ok(assets.insert(Image::new(assets, texture, sampler)?))
    }
}
