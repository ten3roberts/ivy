pub mod emissive;

use ivy_assets::{Asset, AssetCache};
use ivy_wgpu_types::{BindGroupBuilder, BindGroupLayoutBuilder};
use wgpu::{BindGroup, BindGroupLayout, BufferUsages, SamplerDescriptor, ShaderStages, Texture};

use crate::{shader::ShaderPass, types::TypedBuffer};

/// A material for a single pass of the renderer
///
/// Materials use a uniform representation
pub struct RenderMaterial {
    label: String,
    bind_group: Option<BindGroup>,
    layout: Option<BindGroupLayout>,
    shader: Asset<ShaderPass>,
}

impl RenderMaterial {
    pub fn label(&self) -> &str {
        &self.label
    }

    pub fn bind_group(&self) -> Option<&BindGroup> {
        self.bind_group.as_ref()
    }

    pub fn layout(&self) -> Option<&BindGroupLayout> {
        self.layout.as_ref()
    }

    pub fn shader(&self) -> &Asset<ShaderPass> {
        &self.shader
    }
}

pub struct PbrMaterialParams {
    pub albedo: Asset<Texture>,
    pub normal: Asset<Texture>,
    pub metallic_roughness: Asset<Texture>,
    pub ambient_occlusion: Asset<Texture>,
    pub displacement: Asset<Texture>,
    pub roughness_factor: f32,
    pub metallic_factor: f32,
    pub shader: Asset<ShaderPass>,
}

impl PbrMaterialParams {
    pub fn create_material(self, label: String, assets: &AssetCache) -> RenderMaterial {
        let gpu = &assets.service();
        let layout = BindGroupLayoutBuilder::new(label.clone())
            .bind_sampler(ShaderStages::FRAGMENT)
            .bind_texture(ShaderStages::FRAGMENT)
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
            &[PbrMaterialUniformData {
                roughness_factor: self.roughness_factor,
                metallic_factor: self.metallic_factor,
            }],
        );

        let bind_group = BindGroupBuilder::new(&label)
            .bind_sampler(&sampler)
            .bind_texture(&self.albedo.create_view(&Default::default()))
            .bind_texture(&self.normal.create_view(&Default::default()))
            .bind_texture(&self.metallic_roughness.create_view(&Default::default()))
            .bind_texture(&self.ambient_occlusion.create_view(&Default::default()))
            .bind_texture(&self.displacement.create_view(&Default::default()))
            .bind_buffer(&buffer)
            .build(gpu, &layout);

        RenderMaterial {
            label,
            bind_group: Some(bind_group),
            layout: Some(layout),
            shader: self.shader,
        }
    }
}

/// Describes a material for shadow pass rendering
///
/// No color is rendered
#[derive(PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ShadowMaterialDesc {}

impl ShadowMaterialDesc {
    pub fn create_material(self, label: String, shader: Asset<ShaderPass>) -> RenderMaterial {
        RenderMaterial {
            label,
            bind_group: None,
            layout: None,
            shader,
        }
    }
}

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Debug)]
pub(crate) struct PbrMaterialUniformData {
    roughness_factor: f32,
    metallic_factor: f32,
}
