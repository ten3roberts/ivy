use std::{convert::Infallible, path::PathBuf};

use image::{DynamicImage, ImageBuffer};
use ivy_assets::{Asset, AssetCache, AssetDesc};
use wgpu::{Texture, TextureFormat, TextureView, TextureViewDescriptor};

use super::Gpu;

pub fn texture_from_image(
    gpu: &Gpu,
    image: &image::DynamicImage,
    format: TextureFormat,
) -> Texture {
    let image = image.to_rgba8();
    let dimensions = image.dimensions();

    let size = wgpu::Extent3d {
        width: dimensions.0,
        height: dimensions.1,
        depth_or_array_layers: 1,
    };

    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        // All textures are stored as 3D, we represent our 2D texture
        // by setting depth to 1.
        size,
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        // TEXTURE_BINDING tells wgpu that we want to use this texture in shaders
        // COPY_DST means that we want to copy data to this texture
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        label: Some("diffuse_texture"),
        view_formats: &[],
    });

    // Write to the texture
    gpu.queue.write_texture(
        // Tells wgpu where to copy the pixel data
        wgpu::ImageCopyTexture {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        // The actual pixel data
        &image,
        // The layout of the texture
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(format.block_copy_size(None).unwrap() * dimensions.0),
            rows_per_image: Some(dimensions.1),
        },
        size,
    );

    texture
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TextureFromPath {
    pub path: PathBuf,
    pub format: TextureFormat,
}

impl TextureFromPath {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            format: TextureFormat::Rgba8UnormSrgb,
        }
    }

    pub fn new_with_format(path: impl Into<PathBuf>, format: TextureFormat) -> Self {
        Self {
            path: path.into(),
            format,
        }
    }
}

impl AssetDesc<Texture> for TextureFromPath {
    type Error = image::ImageError;

    fn load(
        &self,
        assets: &ivy_assets::AssetCache,
    ) -> Result<ivy_assets::Asset<Texture>, Self::Error> {
        let image = image::open(&self.path)?;

        Ok(assets.insert(texture_from_image(&assets.service(), &image, self.format)))
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct DefaultNormalTexture;

impl AssetDesc<Texture> for DefaultNormalTexture {
    type Error = Infallible;

    fn load(&self, assets: &AssetCache) -> Result<Asset<Texture>, Self::Error> {
        assets.try_load(&TextureFromColor {
            color: [127, 127, 255, 255],
            format: wgpu::TextureFormat::Rgba8Unorm,
        })
    }
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct TextureFromColor {
    pub color: [u8; 4],
    pub format: TextureFormat,
}

impl AssetDesc<Texture> for TextureFromColor {
    type Error = Infallible;

    fn load(&self, assets: &AssetCache) -> Result<Asset<Texture>, Infallible> {
        Ok(assets.insert(texture_from_image(
            &assets.service(),
            &DynamicImage::ImageRgba8(ImageBuffer::from_pixel(32, 32, image::Rgba(self.color))),
            self.format,
        )))
    }
}
