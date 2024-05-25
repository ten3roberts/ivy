use ivy_assets::{Asset, AssetCache};
use wgpu::{Sampler, SamplerDescriptor, ShaderStages};

use crate::Gpu;

use super::{
    texture::{Texture, TextureFromColor, TextureFromPath},
    BindGroupBuilder, BindGroupLayoutBuilder,
};

pub struct Material {
    pub albedo: Asset<Texture>,
    bind_group: wgpu::BindGroup,
    sampler: Sampler,
    layout: wgpu::BindGroupLayout,
}

impl Material {
    pub fn new(gpu: &Gpu, albedo: Asset<Texture>) -> Self {
        let layout = BindGroupLayoutBuilder::new("Material")
            .bind_sampler(ShaderStages::FRAGMENT)
            .bind_texture(ShaderStages::FRAGMENT)
            .build(gpu);

        let sampler = gpu.device.create_sampler(&SamplerDescriptor {
            label: "material_sampler".into(),
            ..Default::default()
        });

        let bind_group = BindGroupBuilder::new("Material")
            .bind_sampler(&sampler)
            .bind_texture(&albedo.create_view(&Default::default()))
            .build(gpu, &layout);

        Self {
            albedo,
            bind_group,
            sampler,
            layout,
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

    pub(crate) fn from_gltf(
        gpu: &Gpu,
        assets: &AssetCache,
        material: gltf::Material,
        textures: &[Asset<Texture>],
    ) -> anyhow::Result<Self> {
        let pbr = material.pbr_metallic_roughness();

        let albedo = pbr
            .base_color_texture()
            .map(|info| {
                let index = info.texture().index();
                textures[index].clone()
            })
            .unwrap_or(assets.load(&TextureFromColor(
                pbr.base_color_factor().map(|v| (v * 255.0) as u8),
            )));

        Ok(Self::new(gpu, albedo))
    }
}
