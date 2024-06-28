use std::{borrow::Cow, convert::Infallible, path::PathBuf};

use anyhow::Context;
use glam::uvec2;
use image::{DynamicImage, GenericImageView, ImageBuffer, RgbaImage};
use itertools::Itertools;
use ivy_assets::{Asset, AssetCache, AssetDesc};
use wgpu::{
    BufferUsages, CommandEncoderDescriptor, ComputePassDescriptor, Extent3d, ImageCopyBuffer,
    ImageCopyTexture, ImageDataLayout, Texture, TextureFormat, TextureUsages,
    TextureViewDescriptor,
};

use crate::{compute::MipLevelPipeline, BindGroupBuilder, TypedBuffer};

use super::Gpu;

pub struct TextureFromImageDesc {
    pub label: Cow<'static, str>,
    pub format: TextureFormat,
    pub mip_level_count: Option<u32>,
    pub usage: TextureUsages,
}

impl Default for TextureFromImageDesc {
    fn default() -> Self {
        Self {
            label: "TextureFromImage".into(),
            format: TextureFormat::Rgba8UnormSrgb,
            mip_level_count: None,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
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
) -> Texture {
    let image = image.to_rgba8();
    let dimensions = image.dimensions();

    let size = wgpu::Extent3d {
        width: dimensions.0,
        height: dimensions.1,
        depth_or_array_layers: 1,
    };

    let mip_level_count = desc
        .mip_level_count
        .unwrap_or_else(|| max_mip_levels(dimensions.0, dimensions.1));

    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        size,
        mip_level_count,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: desc.format,
        usage: desc.usage | TextureUsages::COPY_DST,
        label: Some(desc.label.as_ref()),
        view_formats: &[TextureFormat::Rgba8Unorm],
    });

    let mut mip_level_images = vec![image];

    assert!(mip_level_count > 0);
    for level in 1..mip_level_count {
        tracing::info!(level, "computing mip level");
        let prev = &mip_level_images[level as usize - 1];
        assert!(prev.width() >= 1 && prev.height() >= 2);
        let image = image::imageops::resize(
            prev,
            prev.width() / 2,
            prev.height() / 2,
            image::imageops::FilterType::Triangle,
        );

        mip_level_images.push(image);
    }

    let mut mipped_size = size;
    for level in 0..mip_level_count {
        // Write to the texture
        gpu.queue.write_texture(
            // Tells wgpu where to copy the pixel data
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: level,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            // The actual pixel data
            &mip_level_images[level as usize],
            // The layout of the texture
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(mipped_size.width * desc.format.block_copy_size(None).unwrap()),
                rows_per_image: Some(mipped_size.height),
            },
            mipped_size,
        );

        mipped_size.width = mipped_size.width.max(1) / 2;
        mipped_size.height = mipped_size.height.max(1) / 2;
    }

    texture
}

fn generate_mip_levels(
    gpu: &Gpu,
    assets: &AssetCache,
    extent: Extent3d,
    texture: &Texture,
    mip_level_count: u32,
) {
    let pipeline = assets.load(&MipLevelPipeline);

    let bind_group_layout = pipeline.get_bind_group_layout(0);

    let bind_groups = (1..mip_level_count)
        .map(|level| {
            let input = texture.create_view(&TextureViewDescriptor {
                mip_level_count: Some(1),
                base_mip_level: level - 1,
                format: Some(TextureFormat::Rgba8Unorm),
                ..Default::default()
            });

            let output = texture.create_view(&TextureViewDescriptor {
                mip_level_count: Some(1),
                base_mip_level: level,
                format: Some(TextureFormat::Rgba8Unorm),
                ..Default::default()
            });
            BindGroupBuilder::new("mipmap_generator")
                .bind_texture(&input)
                .bind_texture(&output)
                .build(gpu, &bind_group_layout)
        })
        .collect_vec();

    let invocation_count = uvec2(extent.width / 2, extent.height / 2);
    let workgroups_per_dim = 8;
    let workgroup_count = (invocation_count + workgroups_per_dim - 1) / workgroups_per_dim;
    tracing::info!(%workgroup_count, "dispatching workgroups");

    let mut encoder = gpu
        .device
        .create_command_encoder(&CommandEncoderDescriptor {
            label: "compute_mipmaps".into(),
        });

    {
        let mut compute_pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: "compute_mipmaps".into(),
            timestamp_writes: None,
        });

        for (level, bind_group) in (1..mip_level_count).zip(&bind_groups) {
            compute_pass.set_pipeline(&pipeline);
            tracing::info!("computing level");
            compute_pass.set_bind_group(0, bind_group, &[]);

            compute_pass.dispatch_workgroups(workgroup_count.x, workgroup_count.y, 1);
        }
    }

    gpu.queue.submit([encoder.finish()]);
}

async fn read_texture(gpu: &Gpu, texture: &Texture, mip_level: u32) -> anyhow::Result<RgbaImage> {
    anyhow::ensure!(
        mip_level < texture.mip_level_count(),
        "Mip level out of range"
    );

    let extent = Extent3d {
        width: texture.size().width / (1 << mip_level),
        height: texture.size().height / (1 << mip_level),
        depth_or_array_layers: 1,
    };

    tracing::info!(?extent, "reading back texture");
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
            origin: Default::default(),
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

    let image = image::RgbaImage::from_vec(extent.width, extent.height, mapped.to_vec())
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

        Ok(assets.insert(texture_from_image(
            &assets.service(),
            assets,
            &image,
            TextureFromImageDesc {
                label: self.path.display().to_string().into(),
                format: self.format,
                ..Default::default()
            },
        )))
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
            assets,
            &DynamicImage::ImageRgba8(ImageBuffer::from_pixel(32, 32, image::Rgba(self.color))),
            TextureFromImageDesc {
                format: self.format,
                mip_level_count: Some(1),
                label: "TextureFromColor".into(),
                ..Default::default()
            },
        )))
    }
}

#[test]
fn load_mips() {
    tracing_subscriber::fmt::init();
    futures::executor::block_on(async {
        tracing::info!("loading image");
        let image = image::open("../assets/textures/statue.jpg").unwrap();

        let gpu = Gpu::headless().await;

        let assets = AssetCache::new();
        assets.register_service(gpu.clone());

        tracing::info!("creating image");
        let texture = texture_from_image(
            &gpu,
            &assets,
            &image,
            TextureFromImageDesc {
                label: "test_image".into(),
                format: TextureFormat::Rgba8UnormSrgb,
                mip_level_count: None,
                usage: TextureUsages::COPY_SRC,
            },
        );

        tracing::info!("reading back texture");
        let mip = read_texture(&gpu, &texture, 2).await.unwrap();

        mip.save("../assets/textures/mip_output.png").unwrap();
    });
}
