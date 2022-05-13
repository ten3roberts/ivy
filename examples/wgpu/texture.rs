use std::{
    num::NonZeroU32,
    ops::Deref,
    path::Path,
    sync::{atomic::AtomicU32, Arc},
};

use image::RgbaImage;
use wgpu::{
    ImageCopyTexture, TextureDescriptor, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor,
};

use crate::Gpu;

static TEXTURE_ID: AtomicU32 = AtomicU32::new(0);
#[derive(Debug)]
pub struct Texture {
    id: u32,
    gpu: Arc<Gpu>,
    texture: wgpu::Texture,
}

impl Deref for Texture {
    type Target = wgpu::Texture;

    fn deref(&self) -> &Self::Target {
        &self.texture
    }
}

impl Texture {
    pub async fn from_path(
        gpu: Arc<Gpu>,
        path: impl AsRef<Path>,
        info: &TextureInfo,
    ) -> crate::Result<Self> {
        let path = path.as_ref();
        let buf = tokio::fs::read(path).await?;
        let image = image::load_from_memory(&buf)?;

        Ok(Self::from_image(
            gpu,
            &*path.to_string_lossy(),
            image.into_rgba8(),
            info,
        ))
    }

    pub fn from_image(gpu: Arc<Gpu>, label: &str, image: RgbaImage, info: &TextureInfo) -> Self {
        let (w, h) = image.dimensions();

        let size = wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        };

        let texture = gpu.device().create_texture(&TextureDescriptor {
            label: Some(&label),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: info.format,
            usage: info.usage,
        });

        gpu.queue().write_texture(
            ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &*image,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: NonZeroU32::new(4 * w),
                rows_per_image: NonZeroU32::new(h),
            },
            size,
        );

        Texture {
            id: TEXTURE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            gpu,
            texture,
        }
    }

    pub fn create_view(&self, info: &TextureViewDescriptor) -> TextureView {
        self.texture.create_view(info)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TextureInfo {
    pub format: TextureFormat,
    pub usage: TextureUsages,
}

impl Default for TextureInfo {
    fn default() -> Self {
        Self {
            format: TextureFormat::Rgba8UnormSrgb,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        }
    }
}
