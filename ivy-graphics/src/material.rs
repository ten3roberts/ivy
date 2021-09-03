use std::{slice, sync::Arc};

use crate::Result;
use ash::vk::{DescriptorSet, ShaderStageFlags};
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{
    descriptors::{DescriptorBuilder, IntoSet},
    Buffer, Sampler, Texture, VulkanContext,
};

/// A material contains shader properties such as albedo and other parameters. A material does not
/// define the graphics pipeline nor shaders as that is per pass dependent.
pub struct Material {
    set: DescriptorSet,
    albedo: Handle<Texture>,
    sampler: Handle<Sampler>,
    roughness: f32,
    metallic: f32,
    buffer: Buffer,
}

impl Material {
    /// Creates a new material with albedo using the provided sampler
    pub fn new(
        context: Arc<VulkanContext>,
        resources: &Resources,
        albedo: Handle<Texture>,
        sampler: Handle<Sampler>,
        roughness: f32,
        metallic: f32,
    ) -> Result<Self> {
        let buffer = Buffer::new(
            context.clone(),
            ivy_vulkan::BufferType::Uniform,
            ivy_vulkan::BufferAccess::Staged,
            &[MaterialData {
                roughness,
                metallic,
            }],
        )?;

        let set = DescriptorBuilder::new()
            .bind_combined_image_sampler(
                0,
                ShaderStageFlags::FRAGMENT,
                resources.get(albedo)?.image_view(),
                resources.get(sampler)?.sampler(),
            )
            .bind_buffer(1, ShaderStageFlags::FRAGMENT, &buffer)
            .build(&context)?;

        Ok(Self {
            set,
            albedo,
            sampler,
            roughness,
            metallic,
            buffer,
        })
    }

    pub fn albedo(&self) -> Handle<Texture> {
        self.albedo
    }

    pub fn sampler(&self) -> Handle<Sampler> {
        self.sampler
    }

    /// Get the material's roughness.
    pub fn roughness(&self) -> f32 {
        self.roughness
    }

    /// Get the material's metallic.
    pub fn metallic(&self) -> f32 {
        self.metallic
    }

    /// Get a reference to the material's buffer.
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }
}

impl IntoSet for Material {
    fn set(&self, _: usize) -> DescriptorSet {
        self.set
    }

    fn sets(&self) -> &[DescriptorSet] {
        slice::from_ref(&self.set)
    }
}

#[repr(C)]
struct MaterialData {
    roughness: f32,
    metallic: f32,
}
