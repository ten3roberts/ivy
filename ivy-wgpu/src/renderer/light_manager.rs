use std::iter::repeat;

use flax::{FetchExt, Query};
use glam::{vec3, Mat4, Vec2, Vec3, Vec4};
use itertools::Itertools;
use ivy_core::world_transform;
use ivy_input::Stimulus;
use ivy_wgpu_types::{BindGroupBuilder, BindGroupLayoutBuilder, Gpu, TypedBuffer};
use wgpu::{
    BindGroup, BindGroupLayout, BufferUsages, SamplerDescriptor, ShaderStages,
    TextureViewDescriptor, TextureViewDimension,
};

use crate::{
    components::{light_data, light_kind, light_shadow_data},
    rendergraph::{BufferHandle, NodeUpdateContext, TextureHandle},
};

use super::shadowmapping::LightShadowData;

pub struct LightManager {
    layout: BindGroupLayout,
    bind_group: Option<BindGroup>,
    light_buffer: TypedBuffer<LightData>,
    shadow_camera_buffer: BufferHandle,
    shadow_maps: TextureHandle,
}

impl LightManager {
    pub fn new(
        gpu: &Gpu,
        shadow_maps: TextureHandle,
        shadow_camera_buffer: BufferHandle,
        max_lights: usize,
    ) -> Self {
        let light_buffer = TypedBuffer::new_uninit(
            gpu,
            "light_buffer",
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            max_lights,
        );

        let layout = BindGroupLayoutBuilder::new("LightManager")
            .bind_storage_buffer(ShaderStages::FRAGMENT)
            .bind_storage_buffer(ShaderStages::FRAGMENT)
            .bind_texture_depth_array(ShaderStages::FRAGMENT)
            .bind_sampler_comparison(ShaderStages::FRAGMENT)
            .build(gpu);

        Self {
            light_buffer,
            shadow_maps,
            layout,
            bind_group: None,
            shadow_camera_buffer,
        }
    }

    pub fn update(&mut self, ctx: &NodeUpdateContext) -> anyhow::Result<()> {
        self.bind_group.get_or_insert_with(|| {
            let shadow_sampler = ctx.gpu.device.create_sampler(&SamplerDescriptor {
                label: "shadow sampler".into(),
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::FilterMode::Nearest,
                compare: Some(wgpu::CompareFunction::Less),
                anisotropy_clamp: 1,
                border_color: None,
                ..Default::default()
            });

            BindGroupBuilder::new("LightManager")
                .bind_buffer(&self.light_buffer)
                .bind_buffer(ctx.get_buffer(self.shadow_camera_buffer))
                .bind_texture(&ctx.get_texture(self.shadow_maps).create_view(
                    &TextureViewDescriptor {
                        dimension: Some(TextureViewDimension::D2Array),
                        ..Default::default()
                    },
                ))
                .bind_sampler(&shadow_sampler)
                .build(ctx.gpu, &self.layout)
        });

        let light_data = Query::new((
            world_transform(),
            light_data(),
            light_kind(),
            light_shadow_data().opt(),
        ))
        .borrow(ctx.world)
        .iter()
        .map(|(transform, data, kind, shadow_data)| {
            let color = (vec3(data.color.red, data.color.green, data.color.blue) * data.intensity)
                .extend(1.0);

            let position = transform.transform_point3(Vec3::ZERO);
            let direction = transform.transform_vector3(Vec3::Z).normalize();

            let shadow_data = shadow_data.copied().unwrap_or(LightShadowData {
                index: u32::MAX,
                cascade_count: 0,
            });

            LightData {
                position: position.extend(0.0),
                color,
                kind: *kind as u32,
                direction: direction.normalize().extend(0.0),
                padding: Default::default(),
                shadow_index: shadow_data.index,
                shadow_cascades: shadow_data.cascade_count,
            }
        })
        .chain(repeat(LightData::NONE))
        .take(self.light_buffer.len())
        .collect_vec();

        self.light_buffer.write(&ctx.gpu.queue, 0, &light_data);

        Ok(())
    }

    pub fn light_buffer(&self) -> &TypedBuffer<LightData> {
        &self.light_buffer
    }

    pub fn shadow_maps(&self) -> TextureHandle {
        self.shadow_maps
    }

    pub fn shadow_camera_buffer(&self) -> BufferHandle {
        self.shadow_camera_buffer
    }

    pub fn bind_group(&self) -> Option<&BindGroup> {
        self.bind_group.as_ref()
    }

    pub fn layout(&self) -> &BindGroupLayout {
        &self.layout
    }
}

#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct LightData {
    pub kind: u32,
    pub shadow_index: u32,
    pub shadow_cascades: u32,
    pub padding: f32,
    pub direction: Vec4,
    pub position: Vec4,
    pub color: Vec4,
}

impl LightData {
    const NONE: Self = Self {
        kind: u32::MAX,
        shadow_index: u32::MAX,
        shadow_cascades: u32::MAX,
        padding: 0.0,
        direction: Vec4::ZERO,
        position: Vec4::ZERO,
        color: Vec4::ZERO,
    };
}
