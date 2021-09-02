use crate::{components::Position, Result};
use std::{mem::size_of, sync::Arc};

use ash::vk::{DescriptorSet, ShaderStageFlags};
use hecs::World;
use ivy_vulkan::{
    descriptors::{DescriptorAllocator, DescriptorBuilder, DescriptorLayoutCache, IntoSet},
    Buffer, VulkanContext,
};
use ultraviolet::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Light {
    pub radiance: Vec3,
}

impl Light {
    /// Creates a new light from color radience
    pub fn new(radiance: Vec3) -> Self {
        Self { radiance }
    }

    /// Creates a light from color and intensity
    pub fn from_color(intensity: f32, color: Vec3) -> Self {
        Self {
            radiance: intensity * color,
        }
    }
}

pub struct LightManager {
    light_buffers: Vec<(Buffer, Buffer)>,
    sets: Vec<DescriptorSet>,
    // All registered lights. Note: not all lights may be uploaded to the GPU
    lights: Vec<LightData>,

    max_lights: u64,
    num_lights: u64,
    ambient_radience: Vec3,
}

impl LightManager {
    pub fn new(
        context: Arc<VulkanContext>,
        descriptor_layout_cache: &mut DescriptorLayoutCache,
        descriptor_allocator: &mut DescriptorAllocator,
        max_lights: u64,
        ambient_radience: Vec3,
        frames_in_flight: usize,
    ) -> Result<Self> {
        let light_buffers = (0..frames_in_flight)
            .map(|_| -> Result<_> {
                Ok((
                    Buffer::new_uninit(
                        context.clone(),
                        ivy_vulkan::BufferType::Uniform,
                        ivy_vulkan::BufferAccess::MappedPersistent,
                        size_of::<LightSceneData>() as u64,
                    )?,
                    Buffer::new_uninit(
                        context.clone(),
                        ivy_vulkan::BufferType::Storage,
                        ivy_vulkan::BufferAccess::MappedPersistent,
                        size_of::<LightData>() as u64 * max_lights,
                    )?,
                ))
            })
            .collect::<Result<Vec<_>>>()?;

        let device = context.device();

        let sets = light_buffers
            .iter()
            .map(|buffer| {
                DescriptorBuilder::new()
                    .bind_buffer(0, ShaderStageFlags::FRAGMENT, &buffer.0)
                    .bind_buffer(1, ShaderStageFlags::FRAGMENT, &buffer.1)
                    .build(device, descriptor_layout_cache, descriptor_allocator)
                    .map_err(|e| e.into())
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            light_buffers,
            sets,
            lights: Vec::new(),
            num_lights: 0,
            max_lights,
            ambient_radience,
        })
    }

    /// Updates the GPU side data of the world lights.
    /// Each light which has a [`Light`] and [`Position`] will be considered.
    /// The lights will be sorted in reference to centered. If there are more lights than `max_lights`,
    /// then the n closest will be used.
    pub fn update(&mut self, world: &World, center: Vec3, current_frame: usize) -> Result<()> {
        self.lights.clear();
        self.lights
            .extend(
                world
                    .query::<(&Light, &Position)>()
                    .iter()
                    .map(|(_, (light, position))| LightData {
                        position: position.0,
                        radiance: light.radiance,
                        reference_illuminance: (light.radiance / (center - position.0).mag_sq())
                            .mag(),
                        ..Default::default()
                    }),
            );

        self.lights.sort_unstable();
        self.num_lights = self.max_lights.min(self.lights.len() as u64);

        // Use the first `max_lights` lights and upload to gpu
        self.light_buffers[current_frame]
            .1
            .fill(0, &self.lights[0..self.num_lights as usize])?;

        self.light_buffers[current_frame].0.fill(
            0,
            &[LightSceneData {
                num_lights: self.num_lights as u32,
                ambient_radience: self.ambient_radience,
            }],
        )?;

        Ok(())
    }

    /// Get a reference to the light manager's light buffers.
    pub fn buffers(&self) -> &[(Buffer, Buffer)] {
        &self.light_buffers
    }

    pub fn scene_buffer(&self, current_frame: usize) -> &Buffer {
        &self.light_buffers[current_frame].0
    }

    pub fn light_buffer(&self, current_frame: usize) -> &Buffer {
        &self.light_buffers[current_frame].1
    }
}

impl IntoSet for LightManager {
    fn set(&self, current_frame: usize) -> DescriptorSet {
        self.sets[current_frame]
    }

    fn sets(&self) -> &[DescriptorSet] {
        &self.sets
    }
}

/// Per light data
#[repr(C, align(16))]
#[derive(Default, PartialEq, Debug)]
struct LightData {
    position: Vec3,
    reference_illuminance: f32,
    radiance: Vec3,
}

impl std::cmp::Eq for LightData {}

impl std::cmp::PartialOrd for LightData {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.reference_illuminance
            .partial_cmp(&other.reference_illuminance)
    }
}

impl std::cmp::Ord for LightData {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(std::cmp::Ordering::Equal)
    }
}

#[repr(C)]
struct LightSceneData {
    ambient_radience: Vec3,
    num_lights: u32,
}
