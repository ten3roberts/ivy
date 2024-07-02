use ivy_assets::{Asset, AssetDesc};
use ivy_gltf::{GltfMaterial, GltfMaterialRef};
use wgpu::TextureFormat;

use crate::{material::Material, texture::TextureDesc};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MaterialDesc {
    Gltf(GltfMaterial),

    Content(Asset<MaterialData>),
}

impl From<GltfMaterialRef<'_>> for MaterialDesc {
    fn from(v: GltfMaterialRef) -> Self {
        Self::Gltf(v.into())
    }
}

impl From<GltfMaterial> for MaterialDesc {
    fn from(v: GltfMaterial) -> Self {
        Self::Gltf(v)
    }
}

impl From<Asset<MaterialData>> for MaterialDesc {
    fn from(v: Asset<MaterialData>) -> Self {
        Self::Content(v)
    }
}

impl MaterialDesc {
    pub fn gltf(material: impl Into<GltfMaterial>) -> Self {
        Self::Gltf(material.into())
    }

    pub fn content(content: Asset<MaterialData>) -> Self {
        Self::Content(content)
    }
}

pub struct MaterialData {
    albedo: TextureDesc,
    normal: TextureDesc,
    metallic_roughness: TextureDesc,
    roughness: f32,
    metallic: f32,
}

impl MaterialData {
    pub fn new() -> Self {
        Self {
            albedo: TextureDesc::white(),
            normal: TextureDesc::default_normal(),
            metallic_roughness: TextureDesc::white(),
            roughness: 1.0,
            metallic: 1.0,
        }
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
        self.roughness = roughness;
        self
    }

    /// Set the metallic factor
    pub fn with_metallic(mut self, metallic: f32) -> Self {
        self.metallic = metallic;
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
                    &assets.service(),
                    albedo,
                    normal,
                    metallic_roughness,
                    v.roughness,
                    v.metallic,
                )))
            }
        }
    }
}
