use ivy_assets::{Asset, AssetCache};
use ivy_gltf::GltfPrimitive;
use ivy_graphics::mesh::MeshData;

/// Cpu side mesh descriptor
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MeshDesc {
    Gltf(GltfPrimitive),
    Content(Asset<MeshData>),
}

impl From<GltfPrimitive> for MeshDesc {
    fn from(v: GltfPrimitive) -> Self {
        Self::Gltf(v)
    }
}

impl From<Asset<MeshData>> for MeshDesc {
    fn from(v: Asset<MeshData>) -> Self {
        Self::Content(v)
    }
}

impl MeshDesc {
    pub fn gltf(mesh: impl Into<GltfPrimitive>) -> Self {
        Self::Gltf(mesh.into())
    }

    pub fn content(content: Asset<MeshData>) -> Self {
        Self::Content(content)
    }

    pub fn load_data(&self, assets: &AssetCache) -> anyhow::Result<Asset<MeshData>> {
        match self {
            MeshDesc::Gltf(mesh) => assets.try_load(mesh),
            MeshDesc::Content(v) => Ok(v.clone()),
        }
    }
}
