use std::{borrow::Cow, slice};

use crate::{Error, Result};
use ash::vk::{DescriptorSet, ShaderStageFlags};
use ivy_assets::{Asset, AssetCache, AssetKey};
use ivy_vulkan::{
    context::VulkanContextService,
    descriptors::{DescriptorBuilder, IntoSet},
    Buffer, Sampler, SamplerKey, Texture, TextureFromMemory, TextureFromPath,
};

#[cfg(feature = "serialize")]
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Default)]
#[cfg_attr(feature = "serialize", derive(Serialize, Deserialize))]
pub struct MaterialInfo {
    pub albedo: Cow<'static, str>,
    pub normal: Option<Cow<'static, str>>,
    pub sampler: SamplerKey,
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
    albedo: Asset<Texture>,
    normal: Option<Asset<Texture>>,
    sampler: Asset<Sampler>,
    roughness: f32,
    metallic: f32,
    buffer: Buffer,
}

impl Material {
    /// Creates a new material with albedo using the provided sampler
    pub fn new(
        assets: &AssetCache,
        albedo: Asset<Texture>,
        normal: Option<Asset<Texture>>,
        sampler: Asset<Sampler>,
        roughness: f32,
        metallic: f32,
    ) -> Result<Self> {
        let context = assets.service::<VulkanContextService>().context();

        let buffer = Buffer::new(
            context.clone(),
            ivy_vulkan::BufferUsage::UNIFORM_BUFFER,
            ivy_vulkan::BufferAccess::Staged,
            &[MaterialData {
                roughness,
                metallic,
                normal: normal.is_some() as _,
            }],
        )?;

        let set = DescriptorBuilder::new()
            .bind_combined_image_sampler(
                0,
                ShaderStageFlags::FRAGMENT,
                albedo.image_view(),
                sampler.sampler(),
            )
            .bind_combined_image_sampler(
                1,
                ShaderStageFlags::FRAGMENT,
                normal.as_ref().unwrap_or(&albedo).image_view(),
                sampler.sampler(),
            )
            .bind_buffer(2, ShaderStageFlags::FRAGMENT, &buffer)?
            .build(&context)?;

        Ok(Self {
            set,
            albedo,
            normal,
            sampler,
            roughness,
            metallic,
            buffer,
        })
    }

    #[inline]
    pub fn albedo(&self) -> &Asset<Texture> {
        &self.albedo
    }

    #[inline]
    pub fn sampler(&self) -> &Asset<Sampler> {
        &self.sampler
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
        assets: &AssetCache,
        material: gltf::Material,
        textures: &[Asset<Texture>],
    ) -> Result<Self> {
        let pbr_info = material.pbr_metallic_roughness();
        let albedo = pbr_info.base_color_texture();
        let albedo = if let Some(base_color) = albedo {
            textures[base_color.texture().index()].clone()
        } else {
            TextureFromMemory::solid([255; 4]).load(assets)?
        };

        let normal = if let Some(normal) = material.normal_texture() {
            Some(textures[normal.texture().index()].clone())
        } else {
            None
        };

        let sampler: Asset<Sampler> = SamplerKey::default().load(assets)?;

        Self::new(
            assets,
            albedo,
            normal,
            sampler,
            pbr_info.roughness_factor(),
            pbr_info.metallic_factor(),
        )
    }

    /// Get the material's normal.
    pub fn normal(&self) -> Option<&Asset<Texture>> {
        self.normal.as_ref()
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
    normal: i32,
}

impl AssetKey<Material> for MaterialInfo {
    type Error = Error;

    fn load(&self, assets: &AssetCache) -> Result<Asset<Material>> {
        let context = assets.service::<VulkanContextService>().context();
        let sampler = assets.load(&self.sampler);
        let albedo = assets.load(&TextureFromPath(self.albedo.as_ref().into()));

        let normal = self
            .normal
            .clone()
            .map(|v| assets.try_load(&TextureFromPath(v.as_ref().into())))
            .transpose()?;

        Ok(assets.insert(Material::new(
            assets,
            albedo,
            normal,
            sampler,
            self.roughness,
            self.metallic,
        )?))
    }
}
