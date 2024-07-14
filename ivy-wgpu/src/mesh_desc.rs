use glam::{vec2, vec3, I16Vec4, U16Vec4, Vec2, Vec3, Vec4};
use gltf::mesh::util::weights;
use itertools::{izip, Itertools};
use ivy_assets::{Asset, AssetCache};
use ivy_gltf::GltfPrimitive;

use crate::mesh::{SkinnedVertex, Vertex};

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
/// CPU created mesh data
pub struct MeshData {
    vertices: Box<[Vertex]>,
    joints: Vec<U16Vec4>,
    weights: Vec<Vec4>,
    indices: Box<[u32]>,
}

impl MeshData {
    pub fn new(vertices: Box<[Vertex]>, indices: Box<[u32]>) -> Self {
        Self {
            vertices,
            indices,
            joints: Vec::new(),
            weights: Vec::new(),
        }
    }

    pub fn skinned(
        vertices: Box<[Vertex]>,
        indices: Box<[u32]>,
        joints: Vec<U16Vec4>,
        weights: Vec<Vec4>,
    ) -> Self {
        Self {
            vertices,
            indices,
            joints,
            weights,
        }
    }

    pub fn generate_tangents(&mut self) -> anyhow::Result<()> {
        if !mikktspace::generate_tangents(self) {
            anyhow::bail!("Failed to generate tangents for mesh")
        }

        Ok(())
    }

    pub fn vertices(&self) -> &[Vertex] {
        &self.vertices
    }

    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    pub fn skinned_vertices(&self) -> impl Iterator<Item = SkinnedVertex> + '_ {
        assert_eq!(self.vertices.len(), self.weights.len());
        assert_eq!(self.vertices.len(), self.joints.len());
        izip!(&*self.vertices, &self.weights, &self.joints).map(|(v, &weights, &joints)| -> _ {
            SkinnedVertex {
                pos: v.pos,
                tex_coord: v.tex_coord,
                normal: v.normal,
                tangent: v.tangent,
                weights,
                joints: joints.into(),
            }
        })
    }

    pub fn quad() -> Self {
        let vertices = [
            Vertex::new(vec3(-0.5, -0.5, 0.0), vec2(0.0, 1.0), Vec3::ONE),
            Vertex::new(vec3(0.5, -0.5, 0.0), vec2(1.0, 1.0), Vec3::ONE),
            Vertex::new(vec3(0.5, 0.5, 0.0), vec2(1.0, 0.0), Vec3::ONE),
            Vertex::new(vec3(-0.5, 0.5, 0.0), vec2(0.0, 0.0), Vec3::ONE),
        ];

        let indices = [0, 1, 2, 2, 3, 0];

        let mut this = Self {
            vertices: vertices.to_vec().into_boxed_slice(),
            indices: indices.to_vec().into_boxed_slice(),
            joints: Vec::new(),
            weights: Vec::new(),
        };

        this.generate_tangents().unwrap();
        this
    }

    pub fn cube() -> Self {
        #[rustfmt::skip]
        let vertices = [
            Vertex::new(vec3(-0.5, -0.5, -0.5), vec2(0.0, 1.0), vec3(-1.0, -1.0, -1.0).normalize()),
            Vertex::new(vec3(0.5, -0.5, -0.5), vec2(1.0, 1.0), vec3(1.0, -1.0, -1.0).normalize()),
            Vertex::new(vec3(0.5, 0.5, -0.5), vec2(1.0, 0.0), vec3(1.0, 1.0, -1.0).normalize()),
            Vertex::new(vec3(-0.5, 0.5, -0.5), vec2(0.0, 0.0), vec3(-1.0, 1.0, -1.0).normalize()),
            Vertex::new(vec3(-0.5, -0.5, 0.5), vec2(0.0, 1.0), vec3(-1.0, -1.0, 1.0).normalize()),
            Vertex::new(vec3(0.5, -0.5, 0.5), vec2(1.0, 1.0), vec3(1.0, -1.0, 1.0).normalize()),
            Vertex::new(vec3(0.5, 0.5, 0.5), vec2(1.0, 0.0), vec3(1.0, 1.0, 1.0).normalize()),
            Vertex::new(vec3(-0.5, 0.5, 0.5), vec2(0.0, 0.0), vec3(-1.0, 1.0, 1.0).normalize()),
        ];

        let indices = [
            0, 1, 2, 2, 3, 0, 1, 5, 6, 6, 2, 1, 5, 4, 7, 7, 6, 5, 4, 0, 3, 3, 7, 4, 3, 2, 6, 6, 7,
            3, 4, 5, 1, 1, 0, 4,
        ];

        let mut this = Self {
            vertices: vertices.to_vec().into_boxed_slice(),
            indices: indices.to_vec().into_boxed_slice(),
            joints: Vec::new(),
            weights: Vec::new(),
        };

        this.generate_tangents().unwrap();
        this
    }

    pub(crate) fn from_gltf(
        _: &AssetCache,
        primitive: &gltf::Primitive,
        buffer_data: &[gltf::buffer::Data],
    ) -> anyhow::Result<Self> {
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

        let joints = reader
            .read_joints(0)
            .into_iter()
            .flat_map(|v| v.into_u16())
            .map(U16Vec4::from)
            .collect_vec();

        let weights = reader
            .read_weights(0)
            .into_iter()
            .flat_map(|v| v.into_f32())
            .map(Vec4::from)
            .collect_vec();

        let texcoord = reader
            .read_tex_coords(0)
            .into_iter()
            .flat_map(|val| val.into_f32())
            .map(Vec2::from);

        let vertices = izip!(pos, normals, texcoord)
            .map(|(pos, normal, textcoord)| Vertex::new(pos, textcoord, normal))
            .collect_vec();

        let mut this = Self::skinned(
            vertices.into_boxed_slice(),
            indices.into_boxed_slice(),
            joints,
            weights,
        );

        this.generate_tangents()?;
        Ok(this)
    }

    pub fn vertex_from_face(&self, face: usize, vert: usize) -> &Vertex {
        &self.vertices[self.indices[face * 3 + vert] as usize]
    }

    pub fn vertex_from_face_mut(&mut self, face: usize, vert: usize) -> &mut Vertex {
        &mut self.vertices[self.indices[face * 3 + vert] as usize]
    }
}

impl mikktspace::Geometry for MeshData {
    fn num_faces(&self) -> usize {
        assert_eq!(self.indices.len() % 3, 0);
        self.indices.len() / 3
    }

    fn num_vertices_of_face(&self, _: usize) -> usize {
        3
    }

    fn position(&self, face: usize, vert: usize) -> [f32; 3] {
        self.vertex_from_face(face, vert).pos.into()
    }

    fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
        self.vertex_from_face(face, vert).normal.into()
    }

    fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        self.vertex_from_face(face, vert).tex_coord.into()
    }

    fn set_tangent_encoded(&mut self, tangent: [f32; 4], face: usize, vert: usize) {
        self.vertex_from_face_mut(face, vert).tangent = tangent.into();
    }
}
