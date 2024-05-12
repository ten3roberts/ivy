use ivy_assets::Asset;
use wgpu::ShaderStages;

use crate::Gpu;

use super::{texture::Texture, BindGroupBuilder, BindGroupLayoutBuilder};

pub struct Material {
    pub diffuse: Asset<Texture>,
    bind_group: wgpu::BindGroup,
}

impl Material {
    pub fn new(gpu: &Gpu, diffuse: Asset<Texture>) -> Self {
        let layout = BindGroupLayoutBuilder::new("Material")
            .bind_texture(ShaderStages::FRAGMENT)
            .build(&gpu);

        let bind_group = BindGroupBuilder::new("Material")
            .bind_texture(&diffuse.create_view(&Default::default()))
            .build(gpu, &layout);
        Self {
            diffuse,
            bind_group,
        }
    }

    pub fn bind_group(&self) -> &wgpu::BindGroup {
        &self.bind_group
    }
}
