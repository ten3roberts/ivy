use ivy_assets::{Asset, AssetCache};
use ivy_wgpu_types::{BindGroupBuilder, BindGroupLayoutBuilder, TypedBuffer};
use wgpu::{BufferUsages, SamplerDescriptor, ShaderStages, Texture};

use super::{PbrMaterialParams, RenderMaterial};

pub struct PbrEmissiveMaterialParams {
    pub pbr: PbrMaterialParams,
    pub emissive_color: Asset<Texture>,
    pub emissive_factor: f32,
}

impl PbrEmissiveMaterialParams {
    pub fn create_material(self, label: String, assets: &AssetCache) -> RenderMaterial {
        let gpu = &assets.service();
        let layout = BindGroupLayoutBuilder::new(label.clone())
            .bind_sampler(ShaderStages::FRAGMENT)
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_uniform_buffer(ShaderStages::FRAGMENT)
            .build(gpu);

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
            &[PbrEmissiveMaterialUniformData {
                roughness_factor: self.pbr.roughness_factor,
                metallic_factor: self.pbr.metallic_factor,
                emissive_factor: self.emissive_factor,
            }],
        );

        let bind_group = BindGroupBuilder::new(&label)
            .bind_sampler(&sampler)
            .bind_texture(&self.pbr.albedo.create_view(&Default::default()))
            .bind_texture(&self.pbr.normal.create_view(&Default::default()))
            .bind_texture(&self.pbr.metallic_roughness.create_view(&Default::default()))
            .bind_texture(&self.emissive_color.create_view(&Default::default()))
            .bind_buffer(&buffer)
            .build(gpu, &layout);

        RenderMaterial {
            label,
            bind_group: Some(bind_group),
            layout: Some(layout),
            shader: self.pbr.shader,
        }
    }
}

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Debug)]
pub(crate) struct PbrEmissiveMaterialUniformData {
    roughness_factor: f32,
    metallic_factor: f32,
    emissive_factor: f32,
}
