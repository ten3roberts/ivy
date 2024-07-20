use std::iter::repeat;

use flax::{FetchExt, Query};
use glam::{vec3, Mat4, Vec2, Vec3, Vec4};
use itertools::Itertools;
use ivy_core::world_transform;
use ivy_input::Stimulus;
use ivy_wgpu_types::{Gpu, TypedBuffer};
use wgpu::BufferUsages;

use crate::{
    components::{light_data, light_kind, light_shadow_data},
    rendergraph::{NodeUpdateContext, TextureHandle},
};

pub struct LightManager {
    light_buffer: TypedBuffer<LightData>,
    shadow_maps: TextureHandle,
}

impl LightManager {
    pub fn new(gpu: &Gpu, shadow_maps: TextureHandle, max_lights: usize) -> Self {
        let light_buffer = TypedBuffer::new_uninit(
            gpu,
            "light_buffer",
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            max_lights,
        );

        Self {
            light_buffer,
            shadow_maps,
        }
    }

    pub fn update(&mut self, ctx: &NodeUpdateContext) -> anyhow::Result<()> {
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

            let shadow_viewproj;
            let shadow_index;
            match shadow_data {
                Some(v) => {
                    shadow_index = v.index;
                    shadow_viewproj = v.proj * v.view;
                }
                None => {
                    shadow_index = u32::MAX;
                    shadow_viewproj = Mat4::IDENTITY;
                }
            }

            LightData {
                position: position.extend(0.0),
                color,
                kind: *kind as u32,
                direction: direction.normalize().extend(0.0),
                padding: Default::default(),
                shadow_index,
                shadow_viewproj,
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
}

#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct LightData {
    pub kind: u32,
    pub shadow_index: u32,
    pub padding: Vec2,
    pub shadow_viewproj: Mat4,
    pub direction: Vec4,
    pub position: Vec4,
    pub color: Vec4,
}

impl LightData {
    const NONE: Self = Self {
        kind: u32::MAX,
        shadow_index: u32::MAX,
        padding: Vec2::ZERO,
        shadow_viewproj: Mat4::IDENTITY,
        direction: Vec4::ZERO,
        position: Vec4::ZERO,
        color: Vec4::ZERO,
    };
}
