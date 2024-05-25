use std::convert::Infallible;

use glam::{vec2, vec3, Vec3};
use itertools::Itertools;
use ivy_assets::{Asset, AssetKey};

use crate::{
    graphics::{Mesh, Vertex},
    material::MaterialDesc,
};

/// Cpu side mesh descriptor
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum MeshDesc {
    Path(String),
    Content(Asset<MeshData>),
}

impl MeshDesc {
    pub fn path(path: impl Into<String>) -> Self {
        Self::Path(path.into())
    }

    pub fn content(content: Asset<MeshData>) -> Self {
        Self::Content(content)
    }
}

impl AssetKey<Mesh> for MeshDesc {
    type Error = Infallible;

    fn load(
        &self,
        assets: &ivy_assets::AssetCache,
    ) -> Result<ivy_assets::Asset<Mesh>, Self::Error> {
        let mesh = match self {
            MeshDesc::Path(_) => todo!(),
            MeshDesc::Content(v) => Mesh::new(
                &assets.service(),
                v.vertices(),
                v.indices(),
                v.primitives.as_ref().map(|v| {
                    v.iter()
                        .map(|v| crate::graphics::mesh::Primitive {
                            first_index: v.first_index,
                            index_count: v.index_count,
                            material: assets.load(&v.material),
                        })
                        .collect_vec()
                }),
            ),
        };

        Ok(assets.insert(mesh))
    }
}

pub struct Primitive {
    pub first_index: u32,
    pub index_count: u32,
    pub material: Asset<MaterialDesc>,
}

/// CPU created mesh data
pub struct MeshData {
    vertices: Box<[Vertex]>,
    indices: Box<[u32]>,
    primitives: Option<Vec<Primitive>>,
}

impl MeshData {
    pub fn new(vertices: Box<[Vertex]>, indices: Box<[u32]>) -> Self {
        Self {
            vertices,
            indices,
            primitives: None,
        }
    }

    pub fn new_with_primitives(
        vertices: Box<[Vertex]>,
        indices: Box<[u32]>,
        primitives: Vec<Primitive>,
    ) -> Self {
        Self {
            vertices,
            indices,
            primitives: Some(primitives),
        }
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
            primitives: None,
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
            primitives: None,
        }
    }
}
