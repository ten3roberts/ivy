use ivy_assets::Asset;
use wgpu::{Sampler, SamplerDescriptor, ShaderStages};

use crate::Gpu;

use super::{texture::Texture, BindGroupBuilder, BindGroupLayoutBuilder};

pub struct Material {
    pub diffuse: Asset<Texture>,
    bind_group: wgpu::BindGroup,
    sampler: Sampler,
    layout: wgpu::BindGroupLayout,
}

impl Material {
    pub fn new(gpu: &Gpu, diffuse: Asset<Texture>) -> Self {
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
            .bind_texture(&diffuse.create_view(&Default::default()))
            .build(gpu, &layout);

        Self {
            diffuse,
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
}
