use std::{borrow::Cow, convert::Infallible, path::PathBuf};

use anyhow::Context;
use glam::uvec2;
use image::{ColorType, DynamicImage, GenericImageView, ImageBuffer, Rgba, RgbaImage};
use itertools::Itertools;
use ivy_assets::{Asset, AssetCache, AssetDesc};
use wgpu::{
    BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor, Extent3d, ImageCopyBuffer,
    ImageCopyTexture, ImageDataLayout, Origin3d, Texture, TextureFormat, TextureUsages,
    TextureViewDescriptor,
};

use crate::{mipmap::generate_mipmaps, BindGroupBuilder, TypedBuffer};

use super::Gpu;

#[derive(Debug, Clone)]
pub struct TextureFromImageDesc {
    pub label: Cow<'static, str>,
    pub format: TextureFormat,
    pub mip_level_count: Option<u32>,
    pub usage: TextureUsages,
    pub generate_mipmaps: bool,
}

impl Default for TextureFromImageDesc {
    fn default() -> Self {
        Self {
            label: "TextureFromImage".into(),
            format: TextureFormat::Rgba8UnormSrgb,
            mip_level_count: None,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            generate_mipmaps: true,
        }
    }
}

pub fn max_mip_levels(width: u32, height: u32) -> u32 {
    (width.max(height) as f32).log2().ceil() as u32
}

pub fn texture_from_image(
    gpu: &Gpu,
    assets: &AssetCache,
    image: &image::DynamicImage,
    desc: TextureFromImageDesc,
) -> anyhow::Result<Texture> {
    let _span = tracing::debug_span!("texture_from_image", label = %desc.label).entered();

    let image = normalize_image_format(image, desc.format)?;
    let dimensions = image.dimensions();

    let size = wgpu::Extent3d {
        width: dimensions.0,
        height: dimensions.1,
        depth_or_array_layers: 1,
    };

    let mip_level_count = desc
        .mip_level_count
        .unwrap_or_else(|| max_mip_levels(dimensions.0, dimensions.1));

    let mut usage = TextureUsages::COPY_DST | desc.usage;

    if desc.generate_mipmaps {
        usage |= TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING;
    }
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        size,
        mip_level_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: desc.format,
        usage,
        label: Some(desc.label.as_ref()),
        view_formats: &[],
    });

    tracing::trace!( color_format = ?image.color(), "loading image of format");

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
        image.as_bytes(),
        // The layout of the texture
        wgpu::ImageDataLayout {
            offset: 0,
            bytes_per_row: Some(size.width * desc.format.block_copy_size(None).unwrap()),
            rows_per_image: Some(size.height),
        },
        size,
    );

    if desc.generate_mipmaps {
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        generate_mipmaps(gpu, &mut encoder, &texture, mip_level_count, 0);
        gpu.queue.submit([encoder.finish()]);
    }

    Ok(texture)
}

fn normalize_image_format(
    image: &DynamicImage,
    format: TextureFormat,
) -> anyhow::Result<DynamicImage> {
    let image = match format {
        TextureFormat::Rgba8Unorm => image.to_rgba8().into(),
        TextureFormat::Rgba8UnormSrgb => image.to_rgba8().into(),
        TextureFormat::Rgba16Unorm => image.to_rgba16().into(),
        _ => anyhow::bail!("image loading from format {format:?} is not supported"),
    };

    Ok(image)
}

pub async fn read_texture(
    gpu: &Gpu,
    texture: &Texture,
    mip_level: u32,
    array_layer: u32,
    format: ColorType,
) -> anyhow::Result<DynamicImage> {
    anyhow::ensure!(
        mip_level < texture.mip_level_count(),
        "Mip level out of range"
    );

    let extent = Extent3d {
        width: texture.size().width / (1 << mip_level),
        height: texture.size().height / (1 << mip_level),
        depth_or_array_layers: 1,
    };

    let block_size = texture.format().block_copy_size(None).unwrap();

    let byte_size = extent.width as u64 * extent.height as u64 * block_size as u64;

    let bytes_per_row = extent.width * block_size;

    let buffer = TypedBuffer::<u8>::new_uninit(
        gpu,
        "texture_readback",
        BufferUsages::COPY_DST | BufferUsages::MAP_READ,
        byte_size as usize,
    );

    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        ImageCopyTexture {
            texture,
            mip_level,
            origin: Origin3d {
                x: 0,
                y: 0,
                z: array_layer,
            },
            aspect: Default::default(),
        },
        ImageCopyBuffer {
            buffer: buffer.buffer(),
            layout: ImageDataLayout {
                offset: Default::default(),
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: None,
            },
        },
        extent,
    );

    gpu.queue.submit([encoder.finish()]);

    let mapped = buffer.map(gpu, ..).await.unwrap();

    let image = match format {
        ColorType::Rgba8 => {
            image::RgbaImage::from_vec(extent.width, extent.height, mapped.to_vec())
                .map(DynamicImage::from)
        }
        ColorType::Rgba16 => ImageBuffer::<Rgba<u16>, _>::from_vec(
            extent.width,
            extent.height,
            mapped
                .chunks_exact(2)
                .map(|v| {
                    let [l, h] = v else { unreachable!() };
                    u16::from_le_bytes([*l, *h])
                })
                .collect_vec(),
        )
        .map(Into::into),
        _ => anyhow::bail!("unsupported color type"),
    }
    .context("Failed to create image from buffer")?;

    Ok(image)
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

        Ok(assets.insert(
            texture_from_image(
                &assets.service(),
                assets,
                &image,
                TextureFromImageDesc {
                    label: self.path.display().to_string().into(),
                    format: self.format,
                    ..Default::default()
                },
            )
            .unwrap(),
        ))
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
        Ok(assets.insert(
            texture_from_image(
                &assets.service(),
                assets,
                &DynamicImage::ImageRgba8(ImageBuffer::from_pixel(32, 32, image::Rgba(self.color))),
                TextureFromImageDesc {
                    format: self.format,
                    mip_level_count: Some(1),
                    label: "TextureFromColor".into(),
                    ..Default::default()
                },
            )
            .unwrap(),
        ))
    }
}
