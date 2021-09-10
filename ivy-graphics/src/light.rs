use crate::Result;
use std::{mem::size_of, sync::Arc};

use ash::vk::{DescriptorSet, ShaderStageFlags};
use hecs::World;
use ivy_core::Position;
use ivy_vulkan::{
    descriptors::{DescriptorBuilder, IntoSet},
    Buffer, VulkanContext,
};
use ultraviolet::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointLight {
    pub radiance: Vec3,
}

impl PointLight {
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
    scene_buffers: Vec<Buffer>,
    light_buffers: Vec<Buffer>,
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
        max_lights: u64,
        ambient_radience: Vec3,
        frames_in_flight: usize,
    ) -> Result<Self> {
        let scene_buffers = (0..frames_in_flight)
            .map(|_| -> Result<_> {
                Buffer::new_uninit(
                    context.clone(),
                    ivy_vulkan::BufferType::Uniform,
                    ivy_vulkan::BufferAccess::MappedPersistent,
                    size_of::<LightSceneData>() as u64,
                )
                .map_err(|e| e.into())
            })
            .collect::<Result<Vec<_>>>()?;

        let light_buffers = (0..frames_in_flight)
            .map(|_| -> Result<_> {
                Buffer::new_uninit(
                    context.clone(),
                    ivy_vulkan::BufferType::Storage,
                    ivy_vulkan::BufferAccess::MappedPersistent,
                    size_of::<LightData>() as u64 * max_lights,
                )
                .map_err(|e| e.into())
            })
            .collect::<Result<Vec<_>>>()?;

        let sets = scene_buffers
            .iter()
            .zip(&light_buffers)
            .map(|buffer| {
                DescriptorBuilder::new()
                    .bind_buffer(0, ShaderStageFlags::FRAGMENT, buffer.0)
                    .bind_buffer(1, ShaderStageFlags::FRAGMENT, buffer.1)
                    .build(&context)
                    .map_err(|e| e.into())
            })
            .collect::<Result<Vec<_>>>()?;

        Ok(Self {
            scene_buffers,
            light_buffers,
            sets,
            lights: Vec::new(),
            num_lights: 0,
            max_lights,
            ambient_radience,
        })
    }

    /// Updates the GPU side data of the world lights.
    /// Each light which has a [`PointLight`] and [`Position`] will be considered.
    /// The lights will be sorted in reference to centered. If there are more lights than `max_lights`,
    /// then the n closest will be used.
    pub fn update_system(
        &mut self,
        world: &World,
        center: Vec3,
        current_frame: usize,
    ) -> Result<()> {
        self.lights.clear();
        self.lights
            .extend(world.query::<(&PointLight, &Position)>().iter().map(
                |(_, (light, position))| LightData {
                    position: position.0,
                    radiance: light.radiance,
                    reference_illuminance: (light.radiance / (center - position.0).mag_sq()).mag(),
                    ..Default::default()
                },
            ));

        self.lights.sort_unstable();
        self.num_lights = self.max_lights.min(self.lights.len() as u64);

        // Use the first `max_lights` lights and upload to gpu
        self.light_buffers[current_frame].fill(0, &self.lights[0..self.num_lights as usize])?;

        self.scene_buffers[current_frame].fill(
            0,
            &[LightSceneData {
                num_lights: self.num_lights as u32,
                ambient_radience: self.ambient_radience,
            }],
        )?;

        Ok(())
    }

    pub fn update_all_system(world: &World, current_frame: usize) -> Result<()> {
        world
            .query::<(&mut LightManager, &Position)>()
            .iter()
            .try_for_each(|(_, (light_manager, position))| {
                light_manager.update_system(world, position.0, current_frame)
            })
    }

    pub fn scene_buffers(&self) -> &[Buffer] {
        &self.scene_buffers
    }

    pub fn light_buffers(&self) -> &[Buffer] {
        &self.light_buffers
    }

    pub fn scene_buffer(&self, current_frame: usize) -> &Buffer {
        &self.scene_buffers[current_frame]
    }

    pub fn light_buffer(&self, current_frame: usize) -> &Buffer {
        &self.light_buffers[current_frame]
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
