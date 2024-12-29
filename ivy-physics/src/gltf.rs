use anyhow::Context;
use itertools::Itertools;
use ivy_assets::{Asset, AssetDesc};
use ivy_gltf::GltfPrimitive;
use ivy_graphics::mesh::{MeshData, POSITION_ATTRIBUTE};
use rapier3d::prelude::{SharedShape, TriMeshFlags};

/// Create a trimesh collider from provided primitive
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GltfTriMeshDesc {
    pub primitive: GltfPrimitive,
}

impl GltfTriMeshDesc {
    pub fn new(primitive: GltfPrimitive) -> Self {
        Self { primitive }
    }
}

impl AssetDesc<SharedShape> for GltfTriMeshDesc {
    type Error = anyhow::Error;

    fn create(
        &self,
        assets: &ivy_assets::AssetCache,
    ) -> Result<ivy_assets::Asset<SharedShape>, Self::Error> {
        let mesh: Asset<MeshData> = assets.try_load(&self.primitive)?;

        let positions = mesh
            .get_attribute(POSITION_ATTRIBUTE)
            .context("Missing attribute")?;

        let vertices = positions
            .as_vec3()
            .context("Expected attribute of type vec3")?
            .iter()
            .map(|&v| v.into())
            .collect_vec();

        let shape = SharedShape::trimesh_with_flags(
            vertices,
            mesh.indices()
                .chunks(3)
                .map(|v| [v[0], v[1], v[2]])
                .collect_vec(),
            TriMeshFlags::FIX_INTERNAL_EDGES,
        );

        Ok(assets.insert(shape))
    }
}

/// Create a convex mesh collider from provided primitive
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GltfConvexMeshDesc {
    pub primitive: GltfPrimitive,
}

impl GltfConvexMeshDesc {
    pub fn new(primitive: GltfPrimitive) -> Self {
        Self { primitive }
    }
}

impl AssetDesc<SharedShape> for GltfConvexMeshDesc {
    type Error = anyhow::Error;

    fn create(
        &self,
        assets: &ivy_assets::AssetCache,
    ) -> Result<ivy_assets::Asset<SharedShape>, Self::Error> {
        let mesh: Asset<MeshData> = assets.try_load(&self.primitive)?;

        let positions = mesh
            .get_attribute(POSITION_ATTRIBUTE)
            .context("Missing attribute")?;

        let vertices = positions
            .as_vec3()
            .context("Expected attribute of type vec3")?
            .iter()
            .map(|&v| v.into())
            .collect_vec();

        let shape = SharedShape::convex_hull(&vertices).context("Malformed convex mesh")?;

        Ok(assets.insert(shape))
    }
}
