use itertools::Itertools;
use ivy_assets::AssetCache;
use std::collections::BTreeMap;

use glam::{vec2, vec3, U16Vec4, UVec4, Vec2, Vec3, Vec4};
use ivy_profiling::profile_function;
use tracing::field::ValueSet;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum AttributeType {
    U32,
    Vec3,
    Vec2,
    Vec4,
    UVec4,
    U16Vec4,
}

pub enum AttributeValues {
    U32(Vec<u32>),
    Vec3(Vec<Vec3>),
    Vec2(Vec<Vec2>),
    Vec4(Vec<Vec4>),
    UVec4(Vec<UVec4>),
    U16Vec4(Vec<U16Vec4>),
}

impl AttributeValues {
    pub fn ty(&self) -> AttributeType {
        match self {
            AttributeValues::U32(_) => AttributeType::U32,
            AttributeValues::Vec3(_) => AttributeType::Vec3,
            AttributeValues::Vec2(_) => AttributeType::Vec2,
            AttributeValues::Vec4(_) => AttributeType::Vec4,
            AttributeValues::UVec4(_) => AttributeType::UVec4,
            AttributeValues::U16Vec4(_) => AttributeType::U16Vec4,
        }
    }

    pub fn as_u32(&self) -> Option<&Vec<u32>> {
        if let Self::U32(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_vec3(&self) -> Option<&Vec<Vec3>> {
        if let Self::Vec3(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_vec2(&self) -> Option<&Vec<Vec2>> {
        if let Self::Vec2(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_vec4(&self) -> Option<&Vec<Vec4>> {
        if let Self::Vec4(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_uvec4(&self) -> Option<&Vec<UVec4>> {
        if let Self::UVec4(v) = self {
            Some(v)
        } else {
            None
        }
    }

    pub fn as_u16_vec4(&self) -> Option<&Vec<U16Vec4>> {
        if let Self::U16Vec4(v) = self {
            Some(v)
        } else {
            None
        }
    }
}

impl From<Vec<UVec4>> for AttributeValues {
    fn from(v: Vec<UVec4>) -> Self {
        Self::UVec4(v)
    }
}

impl From<Vec<Vec4>> for AttributeValues {
    fn from(v: Vec<Vec4>) -> Self {
        Self::Vec4(v)
    }
}

impl From<Vec<Vec2>> for AttributeValues {
    fn from(v: Vec<Vec2>) -> Self {
        Self::Vec2(v)
    }
}

impl From<Vec<Vec3>> for AttributeValues {
    fn from(v: Vec<Vec3>) -> Self {
        Self::Vec3(v)
    }
}

impl From<Vec<u32>> for AttributeValues {
    fn from(v: Vec<u32>) -> Self {
        Self::U32(v)
    }
}

impl From<Vec<U16Vec4>> for AttributeValues {
    fn from(v: Vec<U16Vec4>) -> Self {
        Self::U16Vec4(v)
    }
}

impl FromIterator<Vec2> for AttributeValues {
    fn from_iter<T: IntoIterator<Item = Vec2>>(iter: T) -> Self {
        Self::Vec2(iter.into_iter().collect_vec())
    }
}

impl FromIterator<Vec3> for AttributeValues {
    fn from_iter<T: IntoIterator<Item = Vec3>>(iter: T) -> Self {
        Self::Vec3(iter.into_iter().collect_vec())
    }
}

impl FromIterator<Vec4> for AttributeValues {
    fn from_iter<T: IntoIterator<Item = Vec4>>(iter: T) -> Self {
        Self::Vec4(iter.into_iter().collect_vec())
    }
}

impl FromIterator<UVec4> for AttributeValues {
    fn from_iter<T: IntoIterator<Item = UVec4>>(iter: T) -> Self {
        Self::UVec4(iter.into_iter().collect_vec())
    }
}

impl FromIterator<U16Vec4> for AttributeValues {
    fn from_iter<T: IntoIterator<Item = U16Vec4>>(iter: T) -> Self {
        Self::U16Vec4(iter.into_iter().collect_vec())
    }
}
/// CPU created mesh data
pub struct MeshData {
    indices: Vec<u32>,
    attributes: BTreeMap<MeshAttribute, AttributeValues>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MeshAttribute {
    name: &'static str,
    ty: AttributeType,
}

impl MeshAttribute {
    pub const fn new(name: &'static str, ty: AttributeType) -> Self {
        Self { name, ty }
    }
}

pub const POSITION_ATTRIBUTE: MeshAttribute =
    MeshAttribute::new("vertex_position_attribute", AttributeType::Vec3);
pub const TEX_COORD_ATTRIBUTE: MeshAttribute =
    MeshAttribute::new("vertex_tex_coord_attribute", AttributeType::Vec2);
pub const NORMAL_ATTRIBUTE: MeshAttribute =
    MeshAttribute::new("vertex_normal_attribute", AttributeType::Vec3);
pub const JOINT_INDEX_ATTRIBUTE: MeshAttribute =
    MeshAttribute::new("vertex_joint_index_attribute", AttributeType::U16Vec4);
pub const WEIGHT_ATTRIBUTE: MeshAttribute =
    MeshAttribute::new("vertex_weight_attribute", AttributeType::Vec4);
pub const TANGENT_ATTRIBUTE: MeshAttribute =
    MeshAttribute::new("vertex_tangent_attribute", AttributeType::Vec4);

impl MeshData {
    pub fn new() -> Self {
        Self {
            indices: Default::default(),
            attributes: Default::default(),
        }
    }

    pub fn with_indices(mut self, indices: impl IntoIterator<Item = u32>) -> Self {
        self.indices = indices.into_iter().collect_vec();
        self
    }

    pub fn with_attribute<I>(mut self, attribute: MeshAttribute, values: I) -> Self
    where
        I: IntoIterator,
        AttributeValues: FromIterator<I::Item>,
    {
        self.insert_attribute(attribute, values);
        self
    }

    pub fn insert_attribute<I>(&mut self, attribute: MeshAttribute, values: I)
    where
        I: IntoIterator,
        AttributeValues: FromIterator<I::Item>,
    {
        let values = AttributeValues::from_iter(values);

        assert_eq!(
            attribute.ty,
            values.ty(),
            "Values must be of the same type as the attribute {:?}. Expected: {:?}, found: {:?}",
            attribute.name,
            attribute.ty,
            values.ty()
        );

        self.attributes.insert(attribute, values);
    }

    pub fn get_attribute(&self, attribute: MeshAttribute) -> Option<&AttributeValues> {
        self.attributes.get(&attribute)
    }

    pub fn unskinned(
        indices: impl IntoIterator<Item = u32>,
        positions: impl IntoIterator<Item = Vec3>,
        tex_coords: impl IntoIterator<Item = Vec2>,
        normals: impl IntoIterator<Item = Vec3>,
    ) -> Self {
        Self::new()
            .with_indices(indices)
            .with_attribute(POSITION_ATTRIBUTE, positions)
            .with_attribute(TEX_COORD_ATTRIBUTE, tex_coords)
            .with_attribute(NORMAL_ATTRIBUTE, normals)
    }

    pub fn skinned(
        indices: impl IntoIterator<Item = u32>,
        positions: impl IntoIterator<Item = Vec3>,
        tex_coords: impl IntoIterator<Item = Vec2>,
        normals: impl IntoIterator<Item = Vec3>,
        joints: impl IntoIterator<Item = U16Vec4>,
        weights: impl IntoIterator<Item = Vec4>,
    ) -> Self {
        Self::new()
            .with_indices(indices)
            .with_attribute(POSITION_ATTRIBUTE, positions)
            .with_attribute(TEX_COORD_ATTRIBUTE, tex_coords)
            .with_attribute(NORMAL_ATTRIBUTE, normals)
            .with_attribute(JOINT_INDEX_ATTRIBUTE, joints)
            .with_attribute(WEIGHT_ATTRIBUTE, weights)
    }

    pub fn generate_tangents(&mut self) -> anyhow::Result<()> {
        profile_function!();
        let positions = self
            .get_attribute(POSITION_ATTRIBUTE)
            .unwrap()
            .as_vec3()
            .unwrap();
        let tex_coords = self
            .get_attribute(TEX_COORD_ATTRIBUTE)
            .unwrap()
            .as_vec2()
            .unwrap();
        let normals = self
            .get_attribute(NORMAL_ATTRIBUTE)
            .unwrap()
            .as_vec3()
            .unwrap();

        let mut tangents = vec![Vec4::ZERO; positions.len()];
        if !mikktspace::generate_tangents(&mut MikktWrapper {
            indices: &self.indices,
            positions: &positions,
            normals: &normals,
            tex_coords: &tex_coords,
            tangents: &mut tangents,
        }) {
            anyhow::bail!("Failed to generate tangents for mesh")
        }

        self.insert_attribute(TANGENT_ATTRIBUTE, tangents);

        Ok(())
    }

    // pub fn vertices(&self) -> &[Vertex] {
    //     &self.vertices
    // }

    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    // pub fn skinned_vertices(&self) -> impl Iterator<Item = SkinnedVertex> + '_ {
    //     assert_eq!(self.vertices.len(), self.weights.len());
    //     assert_eq!(self.vertices.len(), self.joints.len());
    //     izip!(&*self.vertices, &self.weights, &self.joints).map(|(v, &weights, &joints)| -> _ {
    //         SkinnedVertex {
    //             pos: v.pos,
    //             tex_coord: v.tex_coord,
    //             normal: v.normal,
    //             tangent: v.tangent,
    //             weights,
    //             joints: joints.into(),
    //         }
    //     })
    // }

    pub fn quad() -> Self {
        let positions = [
            vec3(-1.0, -1.0, 0.0),
            vec3(1.0, -1.0, 0.0),
            vec3(1.0, 1.0, 0.0),
            vec3(-1.0, 1.0, 0.0),
        ];

        let tex_coords = [
            vec2(0.0, 1.0),
            vec2(1.0, 1.0),
            vec2(1.0, 0.0),
            vec2(0.0, 0.0),
        ];

        let normals = [Vec3::ONE, Vec3::ONE, Vec3::ONE, Vec3::ONE];

        let indices = [0, 1, 2, 2, 3, 0];

        let mut this = Self::unskinned(indices, positions, tex_coords, normals);
        this.generate_tangents().unwrap();
        this
    }
}

struct MikktWrapper<'a> {
    indices: &'a [u32],
    positions: &'a [Vec3],
    normals: &'a [Vec3],
    tex_coords: &'a [Vec2],
    tangents: &'a mut Vec<Vec4>,
}

impl mikktspace::Geometry for MikktWrapper<'_> {
    fn num_faces(&self) -> usize {
        assert_eq!(self.indices.len() % 3, 0);
        self.indices.len() / 3
    }

    fn num_vertices_of_face(&self, _: usize) -> usize {
        3
    }

    fn position(&self, face: usize, vert: usize) -> [f32; 3] {
        self.positions[self.indices[face * 3 + vert] as usize].into()
    }

    fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
        self.normals[self.indices[face * 3 + vert] as usize].into()
    }

    fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        self.tex_coords[self.indices[face * 3 + vert] as usize].into()
    }

    fn set_tangent_encoded(&mut self, tangent: [f32; 4], face: usize, vert: usize) {
        self.tangents[self.indices[face * 3 + vert] as usize] = tangent.into();
    }
}
