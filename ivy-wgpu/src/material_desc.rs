use ivy_assets::{Asset, AssetDesc};
use ivy_gltf::GltfMaterial;
use ivy_graphics::texture::TextureDesc;
use ordered_float::NotNan;
use wgpu::TextureFormat;

use crate::{
    material::{PbrMaterial, PbrMaterialParams},
    texture::TextureAndKindDesc,
};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum MaterialDesc {
    Gltf(GltfMaterial),

    Content(MaterialData),
}

impl From<GltfMaterial> for MaterialDesc {
    fn from(v: GltfMaterial) -> Self {
        Self::Gltf(v)
    }
}

impl From<MaterialData> for MaterialDesc {
    fn from(v: MaterialData) -> Self {
        Self::Content(v)
    }
}

impl MaterialDesc {
    pub fn gltf(material: impl Into<GltfMaterial>) -> Self {
        Self::Gltf(material.into())
    }

    pub fn content(content: MaterialData) -> Self {
        Self::Content(content)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MaterialData {
    label: String,
    albedo: TextureDesc,
    normal: TextureDesc,
    metallic_roughness: TextureDesc,
    ambient_occlusion: TextureDesc,
    displacement: TextureDesc,
    roughness_factor: NotNan<f32>,
    metallic_factor: NotNan<f32>,
}

impl MaterialData {
    pub fn new() -> Self {
        Self {
            albedo: TextureDesc::white(),
            normal: TextureDesc::default_normal(),
            metallic_roughness: TextureDesc::white(),
            ambient_occlusion: TextureDesc::white(),
            displacement: TextureDesc::white(),
            roughness_factor: 1.0.try_into().unwrap(),
            metallic_factor: 1.0.try_into().unwrap(),
            label: "unknown_material".into(),
        }
    }

    /// Set the label
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = label.into();
        self
    }

    /// Set the albedo
    pub fn with_albedo(mut self, albedo: impl Into<TextureDesc>) -> Self {
        self.albedo = albedo.into();
        self
    }

    /// Set the normal
    pub fn with_normal(mut self, normal: impl Into<TextureDesc>) -> Self {
        self.normal = normal.into();
        self
    }

    /// Set the metallic roughness
    pub fn with_metallic_roughness(mut self, metallic_roughness: impl Into<TextureDesc>) -> Self {
        self.metallic_roughness = metallic_roughness.into();
        self
    }

    /// Set the ambient occlusion
    pub fn with_ambient_occlusion(mut self, ambient_occlusion: impl Into<TextureDesc>) -> Self {
        self.ambient_occlusion = ambient_occlusion.into();
        self
    }

    /// Set the displacement
    pub fn with_displacement(mut self, displacement: impl Into<TextureDesc>) -> Self {
        self.displacement = displacement.into();
        self
    }

    /// Set the roughness factor
    pub fn with_roughness_factor(mut self, roughness: f32) -> Self {
        self.roughness_factor = roughness.try_into().unwrap();
        self
    }

    /// Set the metallic factor
    pub fn with_metallic_factor(mut self, metallic: f32) -> Self {
        self.metallic_factor = metallic.try_into().unwrap();
        self
    }
}

impl Default for MaterialData {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetDesc<PbrMaterial> for MaterialDesc {
    type Error = anyhow::Error;

    fn load(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<PbrMaterial>, Self::Error> {
        match self {
            MaterialDesc::Gltf(v) => {
                let document = assets.try_load(v.data())?;
                let material = document
                    .materials
                    .get(v.index())
                    .ok_or_else(|| anyhow::anyhow!("material out of bounds: {}", v.index(),))?
                    .clone();

                Ok(material)
            }
            MaterialDesc::Content(v) => {
                let albedo = assets.try_load(&TextureAndKindDesc::new(
                    v.albedo.clone(),
                    TextureFormat::Rgba8UnormSrgb,
                ))?;

                let normal = assets.try_load(&TextureAndKindDesc::new(
                    v.normal.clone(),
                    TextureFormat::Rgba8Unorm,
                ))?;

                let metallic_roughness = assets.try_load(&TextureAndKindDesc::new(
                    v.metallic_roughness.clone(),
                    TextureFormat::Rgba8Unorm,
                ))?;

                let ambient_occlusion = assets.try_load(&TextureAndKindDesc::new(
                    v.ambient_occlusion.clone(),
                    TextureFormat::Rgba8Unorm,
                ))?;

                let displacement = assets.try_load(&TextureAndKindDesc::new(
                    v.displacement.clone(),
                    TextureFormat::Rgba8Unorm,
                ))?;

                Ok(assets.insert(PbrMaterial::new(
                    v.label.clone(),
                    &assets.service(),
                    PbrMaterialParams {
                        albedo,
                        normal,
                        metallic_roughness,
                        ambient_occlusion,
                        displacement,
                        roughness_factor: *v.roughness_factor,
                        metallic_factor: *v.metallic_factor,
                    },
                )))
            }
        }
    }
}
