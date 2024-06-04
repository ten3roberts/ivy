use std::{iter::repeat};

use glam::{vec2, vec3, Vec2, Vec3};
use itertools::{izip, Itertools};
use ivy_assets::{Asset, AssetCache};
use ivy_gltf::{GltfPrimitive, GltfPrimitiveRef};

use crate::{
    material::MaterialDesc,
    types::{Vertex},
};

/// Cpu side mesh descriptor
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MeshDesc {
    Gltf(GltfPrimitive),
    Content(Asset<MeshData>),
}

impl From<GltfPrimitiveRef<'_>> for MeshDesc {
    fn from(v: GltfPrimitiveRef) -> Self {
        Self::Gltf(v.into())
    }
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

// impl AssetKey<Mesh> for MeshDesc {
//     type Error = anyhow::Error;

//     fn load(
//         &self,
//         assets: &ivy_assets::AssetCache,
//     ) -> Result<ivy_assets::Asset<Mesh>, Self::Error> {
//         match self {
//             MeshDesc::Gltf(mesh) => assets.try_load(mesh).map_err(Into::into),
//             MeshDesc::Content(v) => {
//                 let mesh = Mesh::new(&assets.service(), v.vertices(), v.indices());

//                 Ok(assets.insert(mesh))
//             }
//         }
//     }
// }

pub struct Primitive {
    pub first_index: u32,
    pub index_count: u32,
    pub material: MaterialDesc,
}

/// CPU created mesh data
pub struct MeshData {
    vertices: Box<[Vertex]>,
    indices: Box<[u32]>,
}

impl MeshData {
    pub fn new(vertices: Box<[Vertex]>, indices: Box<[u32]>) -> Self {
        Self { vertices, indices }
    }

    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    pub fn quad() -> Self {
        let vertices = [
            Vertex::new(vec3(-0.5, -0.5, 0.0), vec2(0.0, 1.0), Vec3::ONE),
            Vertex::new(vec3(0.5, -0.5, 0.0), vec2(1.0, 1.0), Vec3::ONE),
            Vertex::new(vec3(0.5, 0.5, 0.0), vec2(1.0, 0.0), Vec3::ONE),
            Vertex::new(vec3(-0.5, 0.5, 0.0), vec2(0.0, 0.0), Vec3::ONE),
        ];

        let indices = [0, 1, 2, 2, 3, 0];

        Self {
            vertices: vertices.to_vec().into_boxed_slice(),
            indices: indices.to_vec().into_boxed_slice(),
        }
    }

    pub fn cube() -> Self {
        let vertices = [
            Vertex::new(vec3(-0.5, -0.5, -0.5), vec2(0.0, 1.0), Vec3::ONE),
            Vertex::new(vec3(0.5, -0.5, -0.5), vec2(1.0, 1.0), Vec3::ONE),
            Vertex::new(vec3(0.5, 0.5, -0.5), vec2(1.0, 0.0), Vec3::ONE),
            Vertex::new(vec3(-0.5, 0.5, -0.5), vec2(0.0, 0.0), Vec3::ONE),
            Vertex::new(vec3(-0.5, -0.5, 0.5), vec2(0.0, 1.0), Vec3::ONE),
            Vertex::new(vec3(0.5, -0.5, 0.5), vec2(1.0, 1.0), Vec3::ONE),
            Vertex::new(vec3(0.5, 0.5, 0.5), vec2(1.0, 0.0), Vec3::ONE),
            Vertex::new(vec3(-0.5, 0.5, 0.5), vec2(0.0, 0.0), Vec3::ONE),
        ];

        let indices = [
            0, 1, 2, 2, 3, 0, 1, 5, 6, 6, 2, 1, 5, 4, 7, 7, 6, 5, 4, 0, 3, 3, 7, 4, 3, 2, 6, 6, 7,
            3, 4, 5, 1, 1, 0, 4,
        ];

        Self {
            vertices: vertices.to_vec().into_boxed_slice(),
            indices: indices.to_vec().into_boxed_slice(),
        }
    }

    pub(crate) fn from_gltf(
        assets: &AssetCache,
        primitive: &gltf::Primitive,
        buffer_data: &[gltf::buffer::Data],
    ) -> Self {
        let reader = primitive.reader(|buffer| Some(&buffer_data[buffer.index()]));

        let indices = reader
            .read_indices()
            .into_iter()
            .flat_map(|val| val.into_u32())
            .collect_vec();

        let pos = reader
            .read_positions()
            .into_iter()
            .flatten()
            .map(Vec3::from);

        let normals = reader.read_normals().into_iter().flatten().map(Vec3::from);

        let texcoord = reader
            .read_tex_coords(0)
            .into_iter()
            .flat_map(|val| val.into_f32())
            .map(Vec2::from);

        let vertices = izip!(pos, normals, texcoord, repeat(Vec3::ZERO))
            .map(|(pos, normal, textcoord, _tangent)| Vertex::new(pos, textcoord, normal))
            .collect_vec();

        Self::new(vertices.into_boxed_slice(), indices.into_boxed_slice())
    }
}
