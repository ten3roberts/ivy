use image::DynamicImage;
use ivy_assets::AsyncAssetExt;
use wgpu::TextureFormat;

pub struct SkyboxConfig {
    pub hdri: Box<dyn AsyncAssetExt<DynamicImage>>,
    pub format: TextureFormat,
}