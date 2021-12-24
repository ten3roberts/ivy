use crate::Result;
use ash::vk::{DescriptorSet, ShaderStageFlags};
use hecs::World;
use ivy_base::Position;
use ivy_vulkan::{
    descriptors::{DescriptorBuilder, IntoSet},
    Buffer, VulkanContext,
};
use ordered_float::OrderedFloat;
use std::sync::Arc;
use ultraviolet::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointLight {
    pub radiance: Vec3,
    // Visible radius of the light source
    pub radius: f32,
}

impl PointLight {
    /// Creates a new light from color radience
    pub fn new(radius: f32, radiance: Vec3) -> Self {
        Self { radius, radiance }
    }

    /// Creates a light from color and intensity
    pub fn from_color(radius: f32, intensity: f32, color: Vec3) -> Self {
        Self {
            radius,
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
                Buffer::new_uninit::<LightSceneData>(
                    context.clone(),
                    ivy_vulkan::BufferUsage::UNIFORM_BUFFER,
                    ivy_vulkan::BufferAccess::Mapped,
                    1,
                )
                .map_err(|e| e.into())
            })
            .collect::<Result<Vec<_>>>()?;

        let light_buffers = (0..frames_in_flight)
            .map(|_| -> Result<_> {
                Buffer::new_uninit::<LightData>(
                    context.clone(),
                    ivy_vulkan::BufferUsage::STORAGE_BUFFER,
                    ivy_vulkan::BufferAccess::Mapped,
                    max_lights,
                )
                .map_err(|e| e.into())
            })
            .collect::<Result<Vec<_>>>()?;

        let sets = scene_buffers
            .iter()
            .zip(&light_buffers)
            .map(|buffer| {
                DescriptorBuilder::new()
                    .bind_buffer(0, ShaderStageFlags::FRAGMENT, buffer.0)?
                    .bind_buffer(1, ShaderStageFlags::FRAGMENT, buffer.1)?
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
        center: Position,
        current_frame: usize,
    ) -> Result<()> {
        self.lights.clear();
        self.lights.extend(
            world
                .query::<(&PointLight, &Position)>()
                .iter()
                .map(|(_, (light, position))| LightData {
                    position: *position,
                    radiance: light.radiance,
                    reference_illuminance: (light.radiance.mag_sq()
                        / (center - *position).mag_sq()),
                    radius: light.radius,
                    ..Default::default()
                })
                .filter(|val| val.reference_illuminance > 0.01),
        );

        self.lights
            .sort_unstable_by_key(|val| -OrderedFloat(val.reference_illuminance));

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
                light_manager.update_system(world, *position, current_frame)
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
    position: Position,
    reference_illuminance: f32,
    radiance: Vec3,
    radius: f32,
}

impl std::cmp::Eq for LightData {}

#[repr(C)]
struct LightSceneData {
    ambient_radience: Vec3,
    num_lights: u32,
}
