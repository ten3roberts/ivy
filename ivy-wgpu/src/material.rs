use ivy_assets::{Asset, AssetKey};

use crate::{graphics::material::Material, texture::TextureDesc};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MaterialDesc {
    diffuse: TextureDesc,
}

impl MaterialDesc {
    pub fn new(diffuse: TextureDesc) -> Self {
        Self { diffuse }
    }
}

impl AssetKey<Material> for Asset<MaterialDesc> {
    type Error = anyhow::Error;

    fn load(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<Material>, Self::Error> {
        let diffuse = assets.try_load(&self.diffuse)?;

        Ok(assets.insert(Material::new(&*assets.service(), diffuse)))
    }
}
