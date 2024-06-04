pub mod components;

use flax::EntityBuilder;
use glam::{Quat, Vec3};
use itertools::Itertools;
use std::{collections::HashMap, path::Path};

use gltf::{Gltf, Mesh, Primitive};
use ivy_assets::{Asset, AssetCache, AssetKey, StoredKey};

/// An in memory representation of a gltf document and binary buffer data
pub struct DocumentData {
    gltf: Gltf,
    named_meshes: HashMap<String, usize>,
    named_materials: HashMap<String, usize>,
    named_nodes: HashMap<String, usize>,

    buffer_data: Vec<gltf::buffer::Data>,
    // buffer_data: Vec<gltf::buffer::Data>,
}

impl DocumentData {
    pub fn buffer_data(&self) -> &[gltf::buffer::Data] {
        &self.buffer_data
    }

    fn mesh(&self, index: usize) -> Option<gltf::Mesh<'_>> {
        self.gltf.document.meshes().nth(index)
    }

    fn meshes(&self) -> impl Iterator<Item = gltf::Mesh<'_>> + '_ {
        self.gltf.document.meshes()
    }

    fn material(&self, index: usize) -> Option<gltf::Material<'_>> {
        self.gltf.document.materials().nth(index)
    }

    fn materials(&self) -> impl Iterator<Item = gltf::Material<'_>> + '_ {
        self.gltf.document.materials()
    }

    fn node(&self, index: usize) -> Option<gltf::Node<'_>> {
        self.gltf.document.nodes().nth(index)
    }

    fn nodes(&self) -> impl Iterator<Item = gltf::Node<'_>> + '_ {
        self.gltf.document.nodes()
    }

    fn primitive(&self, index: (usize, usize)) -> Option<gltf::Primitive<'_>> {
        self.mesh(index.0).and_then(|v| v.primitives().nth(index.1))
    }

    pub fn primitives(&self) -> impl Iterator<Item = gltf::Primitive<'_>> + '_ {
        self.meshes().flat_map(|v| v.primitives())
    }
}

pub struct Document {
    data: Asset<DocumentData>,
}

impl std::ops::Deref for DocumentData {
    type Target = Gltf;

    fn deref(&self) -> &Self::Target {
        &self.gltf
    }
}

impl Document {
    pub fn new(assets: &AssetCache, path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let bytes: Asset<Vec<u8>> = assets.load(path.as_ref());

        let mut gltf = Gltf::from_slice(&bytes)?;

        let buffer_data: Vec<_> = gltf
            .document
            .buffers()
            .map(|v| {
                tracing::info!(?v, "import buffer");
                // TODO: load using assets
                gltf::buffer::Data::from_source_and_blob(v.source(), None, &mut gltf.blob)
            })
            .try_collect()?;

        let named_meshes = gltf
            .document
            .meshes()
            .enumerate()
            .filter_map(|(i, v)| Some((v.name().map(ToString::to_string)?, i)))
            .collect();

        let named_materials = gltf
            .document
            .materials()
            .enumerate()
            .filter_map(|(i, v)| Some((v.name().map(ToString::to_string)?, i)))
            .collect();

        let named_nodes = gltf
            .document
            .nodes()
            .enumerate()
            .filter_map(|(i, v)| Some((v.name().map(ToString::to_string)?, i)))
            .collect();

        Ok(Self {
            data: assets.insert(DocumentData {
                gltf,
                buffer_data,
                named_meshes,
                named_materials,
                named_nodes,
            }),
        })
    }

    pub fn meshes(&self) -> impl Iterator<Item = GltfMeshRef> {
        self.data.meshes().map(|v| GltfMeshRef::new(&self.data, v))
    }

    pub fn materials(&self) -> impl Iterator<Item = GltfMaterialRef> {
        self.data
            .materials()
            .map(|v| GltfMaterialRef::new(&self.data, v))
    }

    pub fn nodes(&self) -> impl Iterator<Item = GltfNodeRef> {
        self.data.nodes().map(|v| GltfNodeRef::new(&self.data, v))
    }

    pub fn data(&self) -> &DocumentData {
        &self.data
    }

    pub fn mesh(&self, index: usize) -> Option<GltfMeshRef> {
        self.data
            .meshes()
            .nth(index)
            .map(|v| GltfMeshRef::new(&self.data, v))
    }

    pub fn material(&self, index: usize) -> Option<GltfMaterialRef> {
        self.data
            .materials()
            .nth(index)
            .map(|v| GltfMaterialRef::new(&self.data, v))
    }

    pub fn node(&self, index: usize) -> Option<GltfNodeRef> {
        self.data
            .nodes()
            .nth(index)
            .map(|v| GltfNodeRef::new(&self.data, v))
    }

    pub fn find_mesh(&self, name: impl AsRef<str>) -> Option<GltfMeshRef> {
        tracing::info!(?self.data.named_meshes);
        self.data
            .named_meshes
            .get(name.as_ref())
            .map(|&index| self.mesh(index).unwrap())
    }

    pub fn find_material(&self, name: impl AsRef<str>) -> Option<GltfMaterialRef> {
        self.data
            .named_materials
            .get(name.as_ref())
            .map(|&index| self.material(index).unwrap())
    }

    pub fn find_node(&self, name: impl AsRef<str>) -> Option<GltfNodeRef> {
        self.data
            .named_nodes
            .get(name.as_ref())
            .map(|&index| self.node(index).unwrap())
    }
}

#[derive(Debug, Clone)]
pub struct GltfPrimitiveRef<'a> {
    data: &'a Asset<DocumentData>,
    // TODO: store gltf tie instead
    value: gltf::Primitive<'a>,
    mesh: GltfMeshRef<'a>,
}

impl<'a> GltfPrimitiveRef<'a> {
    fn new(
        data: &'a Asset<DocumentData>,
        value: gltf::Primitive<'a>,
        mesh: GltfMeshRef<'a>,
    ) -> Self {
        Self { data, value, mesh }
    }

    #[inline]
    pub fn mesh_index(&self) -> usize {
        self.mesh.index()
    }

    /// **Note**: Refers to the index inside the mesh, not globally
    pub fn index(&self) -> usize {
        self.value.index()
    }

    pub fn material(&self) -> GltfMaterialRef<'a> {
        GltfMaterialRef::new(self.data, self.value.material())
    }
}

/// References a mesh in a gltf document
#[derive(Debug, Clone)]
pub struct GltfMeshRef<'a> {
    data: &'a Asset<DocumentData>,
    value: Mesh<'a>,
}

impl<'a> GltfMeshRef<'a> {
    fn new(data: &'a Asset<DocumentData>, mesh: gltf::Mesh<'a>) -> Self {
        Self { data, value: mesh }
    }

    pub fn index(&self) -> usize {
        self.value.index()
    }

    pub fn name(&self) -> Option<&str> {
        self.value.name()
    }

    pub fn primitives(&self) -> impl Iterator<Item = GltfPrimitiveRef<'a>> + '_ {
        self.value
            .primitives()
            .map(|v| GltfPrimitiveRef::new(&self.data, v, self.clone()))
    }
}

/// References a material in a gltf document
#[derive(Debug, Clone)]
pub struct GltfMaterialRef<'a> {
    data: &'a Asset<DocumentData>,
    value: gltf::Material<'a>,
}

impl<'a> GltfMaterialRef<'a> {
    fn new(data: &'a Asset<DocumentData>, value: gltf::Material<'a>) -> Self {
        Self { data, value }
    }

    pub fn index(&self) -> usize {
        self.value.index().unwrap()
    }

    pub fn name(&self) -> Option<&str> {
        self.value.name()
    }
}

/// References a node in a gltf document
#[derive(Debug, Clone)]
pub struct GltfNodeRef<'a> {
    data: &'a Asset<DocumentData>,
    value: gltf::Node<'a>,
}

impl<'a> GltfNodeRef<'a> {
    fn new(data: &'a Asset<DocumentData>, value: gltf::Node<'a>) -> Self {
        Self { data, value }
    }

    pub fn index(&self) -> usize {
        self.value.index()
    }

    pub fn name(&self) -> Option<&str> {
        self.value.name()
    }

    pub fn mesh(&'a self) -> Option<GltfMeshRef<'a>> {
        Some(GltfMeshRef::new(self.data, self.value.mesh()?))
    }

    pub fn children(&'a self) -> impl Iterator<Item = GltfNodeRef<'a>> {
        self.data
            .node(self.index())
            .unwrap()
            .children()
            .map(move |v| GltfNodeRef::new(self.data, v))
    }

    pub fn transform(&self) -> (Vec3, Quat, Vec3) {
        let (pos, rot, scale) = self.value.transform().decomposed();
        (pos.into(), Quat::from_array(rot), scale.into())
    }
}

/// References a mesh primitive in a gltf document
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GltfPrimitive {
    data: Asset<DocumentData>,
    index: (usize, usize),
}

impl GltfPrimitive {
    pub fn material(&self) -> GltfMaterialRef<'_> {
        self.get_ref().material()
    }
}

/// References a mesh in a gltf document
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GltfMesh {
    data: Asset<DocumentData>,
    index: usize,
}

impl GltfMesh {
    pub fn name(&self) -> Option<&str> {
        self.data.mesh(self.index).and_then(|v| v.name())
    }
}

/// References a material in a gltf document
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GltfMaterial {
    data: Asset<DocumentData>,
    index: usize,
}

impl GltfMaterial {
    pub fn name(&self) -> Option<&str> {
        self.data.material(self.index).and_then(|v| v.name())
    }
}

/// References a node in a gltf document
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GltfNode {
    data: Asset<DocumentData>,
    index: usize,
}

impl GltfNode {
    pub fn name(&self) -> Option<&str> {
        self.data.node(self.index).and_then(|v| v.name())
    }

    pub fn mesh(&self) -> Option<GltfMeshRef<'_>> {
        let node = self.data.node(self.index).unwrap();
        Some(GltfMeshRef::new(&self.data, node.mesh()?))
    }

    pub fn children(&self) -> impl Iterator<Item = GltfNodeRef> {
        self.data
            .node(self.index)
            .unwrap()
            .children()
            .map(move |v| GltfNodeRef::new(&self.data, v))
    }
}

macro_rules! gltf_node_impl {
    ($ty: ty, $ref_ty: ident, $name: ident) => {
        impl $ty {
            #[inline]
            pub fn index(&self) -> usize {
                self.index
            }

            pub fn data(&self) -> &Asset<DocumentData> {
                &self.data
            }

            pub fn get_ref(&self) -> $ref_ty {
                $ref_ty {
                    data: &self.data,
                    value: self.data.$name(self.index()).unwrap(),
                }
            }
        }

        impl<'a> $ref_ty<'a> {
            pub fn data(&self) -> &Asset<DocumentData> {
                self.data
            }
        }

        impl From<$ref_ty<'_>> for $ty {
            fn from(v: $ref_ty) -> Self {
                Self {
                    data: v.data.clone(),
                    index: v.index(),
                }
            }
        }
    };
}

gltf_node_impl! { GltfMesh, GltfMeshRef, mesh }
gltf_node_impl! { GltfNode, GltfNodeRef, node }
gltf_node_impl! { GltfMaterial, GltfMaterialRef, material }

impl GltfPrimitive {
    #[inline]
    pub fn mesh_index(&self) -> usize {
        self.index.0
    }

    /// **Note**: Refers to the index inside the mesh, not globally
    pub fn index(&self) -> usize {
        self.index.1
    }

    pub fn data(&self) -> &Asset<DocumentData> {
        &self.data
    }
    pub fn get_ref(&self) -> GltfPrimitiveRef {
        GltfPrimitiveRef {
            data: &self.data,
            value: self.data.primitive(self.index).unwrap(),
            mesh: GltfMeshRef::new(&self.data, self.data.mesh(self.mesh_index()).unwrap()),
        }
    }
}

impl<'a> GltfPrimitiveRef<'a> {
    pub fn data(&self) -> &Asset<DocumentData> {
        self.data
    }
}

impl From<GltfPrimitiveRef<'_>> for GltfPrimitive {
    fn from(v: GltfPrimitiveRef) -> Self {
        Self {
            data: v.data.clone(),
            index: (v.mesh_index(), v.index()),
        }
    }
}
