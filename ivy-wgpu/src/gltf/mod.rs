pub mod node;

use itertools::Itertools;
use ivy_assets::{Asset, AssetCache, AssetDesc};
use ivy_core::profiling::{profile_function, profile_scope};
use ivy_gltf::DocumentData;

use crate::{material::PbrMaterial, Gpu};

/// Contains the gltf data
pub struct Document {
    pub(crate) materials: Vec<Asset<PbrMaterial>>,
}

impl Document {
    fn new(gpu: &Gpu, assets: &AssetCache, data: &DocumentData) -> anyhow::Result<Self> {
        profile_function!();

        let materials: Vec<_> = data
            .document
            .materials()
            .map(|v| {
                profile_scope!("load_material");
                anyhow::Ok(assets.insert(PbrMaterial::from_gltf(gpu, assets, v, data.images())?))
            })
            .try_collect()?;

        Ok(Self { materials })
    }

    pub fn materials(&self) -> &[Asset<PbrMaterial>] {
        &self.materials
    }
}

impl AssetDesc<Document> for Asset<ivy_gltf::Document> {
    type Error = anyhow::Error;

    fn create(&self, assets: &AssetCache) -> Result<Asset<Document>, Self::Error> {
        Ok(assets.insert(Document::new(&assets.service(), assets, self.data())?))
    }
}

impl AssetDesc<Document> for Asset<ivy_gltf::DocumentData> {
    type Error = anyhow::Error;

    fn create(&self, assets: &AssetCache) -> Result<Asset<Document>, Self::Error> {
        Ok(assets.insert(Document::new(&assets.service(), assets, self)?))
    }
}
