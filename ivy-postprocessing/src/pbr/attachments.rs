use ivy_assets::{Asset, AssetCache};
use ivy_core::Extent;
use ivy_graphics::Result;
use ivy_vulkan::{
    context::SharedVulkanContext, Format, ImageUsage, SampleCountFlags, Texture, TextureInfo,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PBRAttachments {
    pub albedo: Asset<Texture>,
    pub position: Asset<Texture>,
    pub normal: Asset<Texture>,
    pub roughness_metallic: Asset<Texture>,
}

impl PBRAttachments {
    pub fn new(context: SharedVulkanContext, assets: &AssetCache, extent: Extent) -> Result<Self> {
        let albedo = assets.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent,
                mip_levels: 1,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT,
                format: Format::R8G8B8A8_SRGB,
                samples: SampleCountFlags::TYPE_1,
            },
        )?);

        let position = assets.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent,
                mip_levels: 1,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT,
                format: Format::R32G32B32A32_SFLOAT,
                samples: SampleCountFlags::TYPE_1,
            },
        )?);

        let normal = assets.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent,
                mip_levels: 1,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT,
                format: Format::R32G32B32A32_SFLOAT,
                samples: SampleCountFlags::TYPE_1,
            },
        )?);

        let roughness_metallic = assets.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent,
                mip_levels: 1,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT,
                format: Format::R8G8_UNORM,
                samples: SampleCountFlags::TYPE_1,
            },
        )?);

        Ok(Self {
            albedo,
            position,
            normal,
            roughness_metallic,
        })
    }

    pub fn as_slice(&self) -> [&Asset<Texture>; 4] {
        [
            &self.albedo,
            &self.position,
            &self.normal,
            &self.roughness_metallic,
        ]
    }
}
