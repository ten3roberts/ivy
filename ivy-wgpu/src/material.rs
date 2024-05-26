use ivy_assets::{Asset, AssetKey};
use ivy_gltf::{GltfMaterial, GltfMaterialRef};

use crate::{graphics::material::Material, texture::TextureDesc};

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
    diffuse: TextureDesc,
}

impl MaterialData {
    pub fn new(diffuse: TextureDesc) -> Self {
        Self { diffuse }
    }
}

impl AssetKey<Material> for MaterialDesc {
    type Error = anyhow::Error;

    fn load(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<Material>, Self::Error> {
        match self {
            MaterialDesc::Gltf(v) => assets.try_load(v).map_err(Into::into),
            MaterialDesc::Content(v) => {
                let diffuse = assets.try_load(&v.diffuse)?;
                Ok(assets.insert(Material::new(&assets.service(), diffuse)))
            }
        }
    }
}
