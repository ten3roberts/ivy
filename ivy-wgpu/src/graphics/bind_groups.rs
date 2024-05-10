use std::borrow::Cow;

use wgpu::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor, BindGroupLayoutEntry,
    BindingResource, BindingType, Buffer, BufferBindingType, Sampler, SamplerBindingType,
    ShaderStages, TextureSampleType, TextureView, TextureViewDimension,
};

use crate::Gpu;

/// Incrementally construct a bind group layout
#[derive(Debug, Clone)]
pub struct BindGroupLayoutBuilder {
    entries: Vec<BindGroupLayoutEntry>,
    label: Cow<'static, str>,
}

impl BindGroupLayoutBuilder {
    pub fn new(label: impl Into<Cow<'static, str>>) -> Self {
        Self {
            entries: Vec::new(),
            label: label.into(),
        }
    }

    pub fn bind(&mut self, visibility: ShaderStages, ty: BindingType) -> &mut Self {
        let binding = self.entries.len() as u32;

        self.entries.push(BindGroupLayoutEntry {
            binding,
            visibility,
            ty,
            count: None,
        });
        self
    }

    pub fn bind_texture(&mut self, visibility: ShaderStages) -> &mut Self {
        self.bind(
            visibility,
            BindingType::Texture {
                sample_type: TextureSampleType::Float { filterable: true },
                view_dimension: TextureViewDimension::D2,
                multisampled: false,
            },
        )
    }

    pub fn bind_sampler(&mut self, visibility: ShaderStages) -> &mut Self {
        self.bind(
            visibility,
            BindingType::Sampler(SamplerBindingType::Filtering),
        )
    }

    pub fn bind_uniform_buffer(&mut self, visibility: ShaderStages) -> &mut Self {
        self.bind(
            visibility,
            BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
        )
    }

    pub fn bind_storage_buffer(&mut self, visibility: ShaderStages) -> &mut Self {
        self.bind(
            visibility,
            BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
        )
    }

    pub fn build(&self, gpu: &Gpu) -> BindGroupLayout {
        gpu.device
            .create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some(self.label.as_ref()),
                entries: &self.entries,
            })
    }
}

#[derive(Debug, Clone)]
pub struct BindGroupBuilder<'a> {
    entries: Vec<BindGroupEntry<'a>>,
    label: &'a str,
}

impl<'a> BindGroupBuilder<'a> {
    pub fn new(label: &'a str) -> Self {
        Self {
            entries: Vec::new(),
            label,
        }
    }

    pub fn bind(&mut self, resource: BindingResource<'a>) -> &mut Self {
        let binding = self.entries.len() as u32;

        self.entries.push(BindGroupEntry { binding, resource });
        self
    }

    pub fn bind_texture(&mut self, view: &'a TextureView) -> &mut Self {
        self.bind(BindingResource::TextureView(view))
    }

    pub fn bind_sampler(&mut self, sampler: &'a Sampler) -> &mut Self {
        self.bind(BindingResource::Sampler(sampler))
    }

    pub fn bind_buffer(&mut self, buffer: &'a Buffer) -> &mut Self {
        self.bind(buffer.as_entire_binding())
    }

    pub fn build(&self, gpu: &Gpu, layout: &BindGroupLayout) -> BindGroup {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some(self.label),
            layout,
            entries: &self.entries,
        })
    }
}
