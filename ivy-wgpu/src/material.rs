use image::DynamicImage;
use ivy_assets::{Asset, AssetCache};
use ivy_graphics::texture::TextureDesc;
use ivy_wgpu_types::{texture::TextureFromColor, BindGroupBuilder, BindGroupLayoutBuilder};
use wgpu::{BufferUsages, Sampler, SamplerDescriptor, ShaderStages, Texture, TextureFormat};

use crate::{
    texture::TextureAndKindDesc,
    types::{texture::DefaultNormalTexture, Gpu, TypedBuffer},
};

pub struct PbrMaterialParams {
    pub albedo: Asset<Texture>,
    pub normal: Asset<Texture>,
    pub metallic_roughness: Asset<Texture>,
    pub ambient_occlusion: Asset<Texture>,
    pub displacement: Asset<Texture>,
    pub roughness_factor: f32,
    pub metallic_factor: f32,
}

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Debug)]
pub struct MaterialUniformData {
    roughness_factor: f32,
    metallic_factor: f32,
}

pub struct PbrMaterial {
    bind_group: wgpu::BindGroup,
    sampler: Sampler,
    layout: wgpu::BindGroupLayout,
}

impl PbrMaterial {
    pub fn new(label: String, gpu: &Gpu, params: PbrMaterialParams) -> Self {
        let layout = BindGroupLayoutBuilder::new(label.clone())
            .bind_sampler(ShaderStages::FRAGMENT)
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_uniform_buffer(ShaderStages::FRAGMENT)
            .build(gpu);

        // TODO: asset
        let sampler = gpu.device.create_sampler(&SamplerDescriptor {
            label: "material_sampler".into(),
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            anisotropy_clamp: 16,
            ..Default::default()
        });

        let buffer = TypedBuffer::new(
            gpu,
            "material_uniforms",
            BufferUsages::UNIFORM,
            &[MaterialUniformData {
                roughness_factor: params.roughness_factor,
                metallic_factor: params.metallic_factor,
            }],
        );

        let bind_group = BindGroupBuilder::new(&label)
            .bind_sampler(&sampler)
            .bind_texture(&params.albedo.create_view(&Default::default()))
            .bind_texture(&params.normal.create_view(&Default::default()))
            .bind_texture(&params.metallic_roughness.create_view(&Default::default()))
            .bind_texture(&params.ambient_occlusion.create_view(&Default::default()))
            .bind_texture(&params.displacement.create_view(&Default::default()))
            .bind_buffer(&buffer)
            .build(gpu, &layout);

        Self {
            bind_group,
            sampler,
            layout,
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
                    TextureFormat::Rgba8UnormSrgb,
                ))
            })
            .unwrap_or_else(|| {
                assets.load(&TextureFromColor {
                    color: pbr.base_color_factor().map(|v| (v * 255.0) as u8),
                    format: TextureFormat::Rgba8UnormSrgb,
                })
            });

        let normal = material
            .normal_texture()
            .map(|info| {
                let index = info.texture().index();
                assets.load(&TextureAndKindDesc::new(
                    TextureDesc::Content(textures[index].clone()),
                    TextureFormat::Rgba8Unorm,
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
                    TextureFormat::Rgba8Unorm,
                ))
            })
            .unwrap_or_else(|| {
                assets.load(&TextureFromColor {
                    color: [0, 255, 255, 0],
                    format: TextureFormat::Rgba8Unorm,
                })
            });

        let plain_white = assets.load(&TextureFromColor {
            color: [255, 255, 255, 255],
            format: TextureFormat::Rgba8Unorm,
        });

        Ok(Self::new(
            "gltf_material".into(),
            gpu,
            PbrMaterialParams {
                albedo,
                normal,
                metallic_roughness,
                ambient_occlusion: plain_white.clone(),
                displacement: plain_white,
                roughness_factor: pbr.roughness_factor(),
                metallic_factor: pbr.metallic_factor(),
            },
        ))
    }
}
