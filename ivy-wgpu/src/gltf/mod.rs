pub mod node;

use itertools::Itertools;
use ivy_assets::{Asset, AssetCache, AssetDesc};
use ivy_core::profiling::{profile_function, profile_scope};
use ivy_gltf::{DocumentData, GltfPrimitive};

use crate::{material::Material, mesh_desc::MeshData, Gpu};

/// Contains the gltf data
pub struct Document {
    pub(crate) mesh_primitives: Vec<Vec<Asset<MeshData>>>,
    pub(crate) materials: Vec<Asset<Material>>,
}

impl Document {
    fn new(gpu: &Gpu, assets: &AssetCache, data: &DocumentData) -> anyhow::Result<Self> {
        profile_function!();

        let materials: Vec<_> = data
            .document
            .materials()
            .map(|v| {
                profile_scope!("load_material");
                anyhow::Ok(assets.insert(Material::from_gltf(gpu, assets, v, data.images())?))
            })
            .try_collect()?;

        let mesh_primitives: Vec<_> = data
            .document
            .meshes()
            .map(|mesh| -> anyhow::Result<Vec<_>> {
                profile_scope!("load_mesh_primitive");
                mesh.primitives()
                    .map(|primitive| {
                        Ok(assets.insert(MeshData::from_gltf(
                            assets,
                            &primitive,
                            data.buffer_data(),
                        )?))
                    })
                    .try_collect()
            })
            .try_collect()?;

        Ok(Self {
            mesh_primitives,
            materials,
        })
    }

    pub fn materials(&self) -> &[Asset<Material>] {
        &self.materials
    }

    pub fn mesh_primitives(&self) -> &[Vec<Asset<MeshData>>] {
        &self.mesh_primitives
    }
}

impl AssetDesc<Document> for Asset<ivy_gltf::Document> {
    type Error = anyhow::Error;

    fn load(&self, assets: &AssetCache) -> Result<Asset<Document>, Self::Error> {
        Ok(assets.insert(Document::new(&assets.service(), assets, self.data())?))
    }
}

impl AssetDesc<Document> for Asset<ivy_gltf::DocumentData> {
    type Error = anyhow::Error;

    fn load(&self, assets: &AssetCache) -> Result<Asset<Document>, Self::Error> {
        Ok(assets.insert(Document::new(&assets.service(), assets, self)?))
    }
}

impl AssetDesc<MeshData> for GltfPrimitive {
    type Error = anyhow::Error;

    fn load(&self, assets: &AssetCache) -> Result<Asset<MeshData>, Self::Error> {
        let document: Asset<Document> = assets.try_load(self.data())?;

        document
            .mesh_primitives
            .get(self.mesh_index())
            .ok_or_else(|| anyhow::anyhow!("mesh out of bounds: {}", self.mesh_index(),))?
            .get(self.index())
            .ok_or_else(|| anyhow::anyhow!("mesh primitive out of bounds: {}", self.index(),))
            .cloned()
    }
}
