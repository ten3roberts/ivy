use anyhow::Context;
use glam::Mat4;
use itertools::Itertools;
use ivy_assets::Asset;
use ivy_gltf::GltfPrimitive;
use ivy_graphics::mesh::{MeshData, POSITION_ATTRIBUTE};
use rapier3d::{
    math::Point,
    prelude::{SharedShape, TriMeshFlags},
};

/// Create a trimesh collider from provided primitive
#[derive(Debug, Clone, PartialEq)]
pub struct GltfTriMeshDesc {
    pub transform: Mat4,
    pub primitives: Vec<GltfPrimitive>,
}

impl GltfTriMeshDesc {
    pub fn create(
        &self,
        assets: &ivy_assets::AssetCache,
    ) -> anyhow::Result<ivy_assets::Asset<SharedShape>> {
        let mut vertices: Vec<Point<f32>> = Vec::new();
        let mut indices = Vec::new();

        for primitive in &self.primitives {
            let mesh: Asset<MeshData> = assets.try_load(primitive)?;

            let positions = mesh
                .get_attribute(POSITION_ATTRIBUTE)
                .context("Missing attribute")?;

            let base_offset = vertices.len() as u32;
            indices.extend(
                mesh.indices()
                    .chunks(3)
                    .map(|v| [v[0] + base_offset, v[1] + base_offset, v[2] + base_offset]),
            );

            vertices.extend(
                positions
                    .as_vec3()
                    .context("Expected attribute of type vec3")?
                    .iter()
                    .map(|&v| Point::from(self.transform.transform_point3(v))),
            );
        }

        let shape =
            SharedShape::trimesh_with_flags(vertices, indices, TriMeshFlags::FIX_INTERNAL_EDGES)?;

        Ok(assets.insert(shape))
    }
}

/// Create a convex mesh collider from provided primitive
#[derive(Debug, Clone, PartialEq)]
pub struct GltfConvexMeshDesc {
    pub transform: Mat4,
    pub primitive: GltfPrimitive,
}

impl GltfConvexMeshDesc {
    pub fn create(
        &self,
        assets: &ivy_assets::AssetCache,
    ) -> anyhow::Result<ivy_assets::Asset<SharedShape>> {
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
