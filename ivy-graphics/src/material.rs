use std::{borrow::Cow, slice, sync::Arc};

use crate::{Error, Result};
use ash::vk::{DescriptorSet, ShaderStageFlags};
use ivy_resources::{Handle, LoadResource, Resources};
use ivy_vulkan::{
    descriptors::{DescriptorBuilder, IntoSet},
    Buffer, Sampler, SamplerInfo, Texture, VulkanContext,
};

#[derive(Debug, Clone, PartialEq, Default)]
pub struct MaterialInfo {
    pub albedo: Cow<'static, str>,
    pub sampler: SamplerInfo,
    pub roughness: f32,
    pub metallic: f32,
}

// Roughness or metallic should not be inf or nan
impl Eq for MaterialInfo {}

impl std::hash::Hash for MaterialInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.albedo.hash(state);
        self.sampler.hash(state);
        ((self.roughness * 100.0) as usize).hash(state);
        ((self.metallic * 100.0) as usize).hash(state);
    }
}

/// A material contains shader properties such as albedo and other parameters. A material does not
/// define the graphics pipeline nor shaders as that is per pass dependent.
/// *Note*: prefer Materials instead to store several materials.
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
        eprintln!("roughness: {}, metallic: {}", roughness, metallic);
        let buffer = Buffer::new(
            context.clone(),
            ivy_vulkan::BufferUsage::UNIFORM_BUFFER,
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
            .bind_buffer(1, ShaderStageFlags::FRAGMENT, &buffer)?
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

    #[inline]
    pub fn albedo(&self) -> Handle<Texture> {
        self.albedo
    }

    #[inline]
    pub fn sampler(&self) -> Handle<Sampler> {
        self.sampler
    }

    /// Get the material's roughness.
    #[inline]
    pub fn roughness(&self) -> f32 {
        self.roughness
    }

    /// Get the material's metallic.
    #[inline]
    pub fn metallic(&self) -> f32 {
        self.metallic
    }

    /// Get a reference to the material's buffer.
    #[inline]
    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn from_gltf(
        context: Arc<VulkanContext>,
        material: gltf::Material,
        textures: &[Handle<Texture>],
        resources: &Resources,
    ) -> Result<Self> {
        let pbr_info = material.pbr_metallic_roughness();
        let albedo = pbr_info.base_color_texture();
        let albedo = if let Some(base_color) = albedo {
            textures[base_color.texture().index()]
        } else {
            resources.default()?
        };

        let sampler: Handle<Sampler> = resources.load(SamplerInfo::default())??;

        Self::new(
            context,
            resources,
            albedo,
            sampler,
            pbr_info.roughness_factor(),
            pbr_info.metallic_factor(),
        )
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

impl LoadResource for Material {
    type Info = MaterialInfo;

    type Error = Error;

    fn load(resources: &Resources, info: &Self::Info) -> Result<Self> {
        let context = resources.get_default::<Arc<VulkanContext>>()?;
        let sampler: Handle<Sampler> = resources.load(info.sampler)??;
        let albedo = resources.load(info.albedo.clone())??;

        Self::new(
            context.clone(),
            resources,
            albedo,
            sampler,
            info.roughness,
            info.metallic,
        )
    }
}
