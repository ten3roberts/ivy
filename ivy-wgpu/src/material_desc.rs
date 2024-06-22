use image::Rgba;
use ivy_assets::{Asset, AssetKey};
use ivy_gltf::{GltfMaterial, GltfMaterialRef};

use crate::{
    material::Material,
    texture::{TextureDesc, TextureKind},
};

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
    metallic_roughness: Option<TextureDesc>,
    roughness: f32,
    metallic: f32,
}

impl MaterialData {
    pub fn new(albedo: TextureDesc, normal: TextureDesc, roughness: f32, metallic: f32) -> Self {
        Self {
            albedo,
            normal,
            roughness,
            metallic,
            metallic_roughness: None,
        }
    }
}

impl AssetKey<Material> for MaterialDesc {
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
                let albedo = v.albedo.load(assets, TextureKind::Srgba)?;
                let normal = v.normal.load(assets, TextureKind::Uniform)?;
                let metallic_roughness = v
                    .metallic_roughness
                    .as_ref()
                    .unwrap_or_else(|| &TextureDesc::Color(Rgba([255, 255, 255, 255])))
                    .load(assets, TextureKind::Uniform)?;

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
