use ivy_base::Extent;
use ivy_graphics::Result;
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{
    context::SharedVulkanContext, Format, ImageUsage, SampleCountFlags, Texture, TextureInfo,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PBRAttachments {
    pub albedo: Handle<Texture>,
    pub position: Handle<Texture>,
    pub normal: Handle<Texture>,
    pub roughness_metallic: Handle<Texture>,
}

impl PBRAttachments {
    pub fn new(
        context: SharedVulkanContext,
        resources: &Resources,
        extent: Extent,
    ) -> Result<Self> {
        let albedo = resources.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent,
                mip_levels: 1,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT,
                format: Format::R8G8B8A8_SRGB,
                samples: SampleCountFlags::TYPE_1,
            },
        )?)?;

        let position = resources.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent,
                mip_levels: 1,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT,
                format: Format::R32G32B32A32_SFLOAT,
                samples: SampleCountFlags::TYPE_1,
            },
        )?)?;

        let normal = resources.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent,
                mip_levels: 1,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT,
                format: Format::R32G32B32A32_SFLOAT,
                samples: SampleCountFlags::TYPE_1,
            },
        )?)?;

        let roughness_metallic = resources.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent,
                mip_levels: 1,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT,
                format: Format::R8G8_UNORM,
                samples: SampleCountFlags::TYPE_1,
            },
        )?)?;

        let trans = resources.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent,
                mip_levels: 1,
                usage: ImageUsage::COLOR_ATTACHMENT | ImageUsage::INPUT_ATTACHMENT,
                format: Format::R8G8B8A8_SRGB,
                samples: SampleCountFlags::TYPE_1,
            },
        )?)?;

        Ok(Self {
            albedo,
            position,
            normal,
            roughness_metallic,
        })
    }

    pub fn as_slice(&self) -> [Handle<Texture>; 4] {
        [
            self.albedo,
            self.position,
            self.normal,
            self.roughness_metallic,
        ]
    }
}
