use ivy_assets::{Asset, AssetDesc};
use ivy_gltf::GltfMaterial;
use ordered_float::NotNan;
use wgpu::TextureFormat;

use crate::{material::Material, texture::TextureDesc};

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
    roughness: NotNan<f32>,
    metallic: NotNan<f32>,
}

impl MaterialData {
    pub fn new() -> Self {
        Self {
            albedo: TextureDesc::white(),
            normal: TextureDesc::default_normal(),
            metallic_roughness: TextureDesc::white(),
            roughness: 1.0.try_into().unwrap(),
            metallic: 1.0.try_into().unwrap(),
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

    /// Set the roughness factor
    pub fn with_roughness(mut self, roughness: f32) -> Self {
        self.roughness = roughness.try_into().unwrap();
        self
    }

    /// Set the metallic factor
    pub fn with_metallic(mut self, metallic: f32) -> Self {
        self.metallic = metallic.try_into().unwrap();
        self
    }
}

impl Default for MaterialData {
    fn default() -> Self {
        Self::new()
    }
}

impl AssetDesc<Material> for MaterialDesc {
    type Error = anyhow::Error;

    fn load(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<Material>, Self::Error> {
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
                let albedo = v.albedo.load(assets, TextureFormat::Rgba8UnormSrgb)?;
                let normal = v.normal.load(assets, TextureFormat::Rgba8Unorm)?;
                let metallic_roughness = v
                    .metallic_roughness
                    .load(assets, TextureFormat::Rgba8Unorm)?;

                Ok(assets.insert(Material::new(
                    v.label.clone(),
                    &assets.service(),
                    albedo,
                    normal,
                    metallic_roughness,
                    *v.roughness,
                    *v.metallic,
                )))
            }
        }
    }
}
