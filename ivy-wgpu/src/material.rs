use image::DynamicImage;
use ivy_assets::{Asset, AssetCache};
use ivy_wgpu_types::{texture::TextureFromColor, BindGroupBuilder, BindGroupLayoutBuilder};
use wgpu::{BufferUsages, Sampler, SamplerDescriptor, ShaderStages, Texture};

use crate::{
    texture::{TextureAndKindDesc, TextureDesc, TextureKind},
    types::{texture::DefaultNormalTexture, Gpu, TypedBuffer},
};

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Debug)]
pub struct MaterialUniformData {
    roughness_factor: f32,
    metallic_factor: f32,
}

pub struct Material {
    pub albedo: Asset<Texture>,
    pub normal: Asset<Texture>,
    pub roughness_factor: f32,
    pub metallic_factor: f32,
    bind_group: wgpu::BindGroup,
    // buffer: TypedBuffer<MaterialUniformData>,
    sampler: Sampler,
    layout: wgpu::BindGroupLayout,
}

impl Material {
    pub fn new(
        gpu: &Gpu,
        albedo: Asset<Texture>,
        normal: Asset<Texture>,
        metallic_roughness: Asset<Texture>,
        roughness_factor: f32,
        metallic_factor: f32,
    ) -> Self {
        let layout = BindGroupLayoutBuilder::new("Material")
            .bind_sampler(ShaderStages::FRAGMENT)
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_uniform_buffer(ShaderStages::FRAGMENT)
            .build(gpu);

        let sampler = gpu.device.create_sampler(&SamplerDescriptor {
            label: "material_sampler".into(),
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let buffer = TypedBuffer::new(
            gpu,
            "material_uniforms",
            BufferUsages::UNIFORM,
            &[MaterialUniformData {
                roughness_factor,
                metallic_factor,
            }],
        );

        let bind_group = BindGroupBuilder::new("Material")
            .bind_sampler(&sampler)
            .bind_texture(&albedo.create_view(&Default::default()))
            .bind_texture(&normal.create_view(&Default::default()))
            .bind_texture(&metallic_roughness.create_view(&Default::default()))
            .bind_buffer(&buffer)
            .build(gpu, &layout);

        Self {
            albedo,
            normal,
            bind_group,
            sampler,
            layout,
            roughness_factor,
            metallic_factor,
            // buffer,
        }
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }

    pub fn sampler(&self) -> &Sampler {
        &self.sampler
    }

    pub fn layout(&self) -> &wgpu::BindGroupLayout {
        &self.layout
    }

    pub fn from_gltf(
        gpu: &Gpu,
        assets: &AssetCache,
        material: gltf::Material,
        textures: &[Asset<DynamicImage>],
    ) -> anyhow::Result<Self> {
        let pbr = material.pbr_metallic_roughness();

        let albedo = pbr
            .base_color_texture()
            .map(|info| {
                let texture = textures[info.texture().index()].clone();
                assets.load(&TextureAndKindDesc::new(
                    TextureDesc::Content(texture),
                    TextureKind::Srgba,
                ))
            })
            .unwrap_or_else(|| {
                assets.load(&TextureFromColor {
                    color: pbr.base_color_factor().map(|v| (v * 255.0) as u8),
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                })
            });

        let normal = material
            .normal_texture()
            .map(|info| {
                let index = info.texture().index();
                assets.load(&TextureAndKindDesc::new(
                    TextureDesc::Content(textures[index].clone()),
                    TextureKind::Uniform,
                ))
            })
            .unwrap_or_else(|| assets.load(&DefaultNormalTexture));

        // R: _
        // G: roughness
        // B: metal
        let metallic_roughness = pbr
            .metallic_roughness_texture()
            .map(|info| {
                let index = info.texture().index();
                assets.load(&TextureAndKindDesc::new(
                    TextureDesc::Content(textures[index].clone()),
                    TextureKind::Uniform,
                ))
            })
            .unwrap_or_else(|| {
                assets.load(&TextureFromColor {
                    color: [0, 255, 255, 0],
                    format: wgpu::TextureFormat::R8Unorm,
                })
            });

        tracing::info!(
            metallic_roughness = pbr.metallic_roughness_texture().is_some(),
            normal_map = material.normal_texture().is_some(),
            roughness_factor = pbr.roughness_factor(),
            metallic_factor = pbr.metallic_factor(),
            "gltf material"
        );

        Ok(Self::new(
            gpu,
            albedo,
            normal,
            metallic_roughness,
            pbr.roughness_factor(),
            pbr.metallic_factor(),
        ))
    }
}
