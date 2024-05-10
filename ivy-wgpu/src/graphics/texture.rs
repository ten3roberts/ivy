use wgpu::{TextureView, TextureViewDescriptor};

use super::Gpu;

/// Higher level wrapper around wgpu::Texture
pub struct Texture {
    texture: wgpu::Texture,
}

impl std::ops::Deref for Texture {
    type Target = wgpu::Texture;

    fn deref(&self) -> &Self::Target {
        &self.texture
    }
}

impl Texture {
    pub fn from_texture(texture: wgpu::Texture) -> Self {
        Self { texture }
    }

    pub fn from_image(gpu: &Gpu, image: &image::DynamicImage) -> Self {
        let image = image.to_rgba8();
        let dimensions = image.dimensions();

        let size = wgpu::Extent3d {
            width: dimensions.0,
            height: dimensions.1,
            depth_or_array_layers: 1,
        };

        let format = wgpu::TextureFormat::Rgba8UnormSrgb;

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

        Self { texture }
    }

    pub fn view(&self, desc: &TextureViewDescriptor) -> TextureView {
        self.texture.create_view(desc)
    }
}
