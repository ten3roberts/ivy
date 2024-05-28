pub mod components;

use flax::EntityBuilder;
use itertools::Itertools;
use std::{collections::HashMap, path::Path};

use gltf::{Gltf, Primitive};
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

    fn primitive(&self, index: usize) -> Option<Primitive<'_>> {
        self.mesh(index).and_then(|v| v.primitives().next())
    }

    fn primitives(&self) -> impl Iterator<Item = Primitive<'_>> + '_ {
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
        self.data.meshes().enumerate().map(|(i, v)| GltfMeshRef {
            data: &self.data,
            index: i,
        })
    }

    pub fn materials(&self) -> impl Iterator<Item = GltfMaterialRef> {
        self.data
            .materials()
            .enumerate()
            .map(|(i, v)| GltfMaterialRef {
                data: &self.data,
                index: i,
            })
    }

    pub fn nodes(&self) -> impl Iterator<Item = GltfNodeRef> {
        self.data.nodes().enumerate().map(|(i, v)| GltfNodeRef {
            data: &self.data,
            index: i,
        })
    }

    pub fn data(&self) -> &DocumentData {
        &self.data
    }

    pub fn mesh(&self, index: usize) -> Option<GltfMeshRef> {
        self.data.meshes().nth(index).map(|v| GltfMeshRef {
            data: &self.data,
            index,
        })
    }

    pub fn material(&self, index: usize) -> Option<GltfMaterialRef> {
        self.data.materials().nth(index).map(|v| GltfMaterialRef {
            data: &self.data,
            index,
        })
    }

    pub fn node(&self, index: usize) -> Option<GltfNodeRef> {
        self.data.nodes().nth(index).map(|v| GltfNodeRef {
            data: &self.data,
            index,
        })
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

pub struct GltfPrimitiveRef<'a> {
    data: &'a Asset<DocumentData>,
    primitive: Primitive<'a>,
}

impl<'a> GltfPrimitiveRef<'a> {
    pub fn material(&self) -> GltfMaterialRef {
        GltfMaterialRef {
            data: self.data,
            index: self.primitive.material().index().unwrap(),
        }
    }
}

impl From<GltfPrimitiveRef<'_>> for GltfPrimitive {
    fn from(v: GltfPrimitiveRef) -> Self {
        Self {
            data: v.data.clone(),
            index: v.primitive.index(),
        }
    }
}

/// References a mesh in a gltf document
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GltfMeshRef<'a> {
    data: &'a Asset<DocumentData>,
    index: usize,
}

impl<'a> GltfMeshRef<'a> {
    pub fn name(&self) -> Option<&str> {
        self.data.mesh(self.index).unwrap().name()
    }

    fn from_gltf(data: &'a Asset<DocumentData>, mesh: gltf::Mesh<'a>) -> Self {
        Self {
            data,
            index: mesh.index(),
        }
    }

    pub fn primitives(&self) -> impl Iterator<Item = GltfPrimitiveRef<'a>> {
        self.data
            .mesh(self.index)
            .unwrap()
            .primitives()
            .map(|v| GltfPrimitiveRef {
                data: self.data,
                primitive: v,
            })
    }

    pub fn data(&self) -> &Asset<DocumentData> {
        self.data
    }
}

/// References a material in a gltf document
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GltfMaterialRef<'a> {
    data: &'a Asset<DocumentData>,
    index: usize,
}

impl<'a> GltfMaterialRef<'a> {
    pub fn name(&self) -> Option<&str> {
        self.data.material(self.index).unwrap().name()
    }

    pub fn data(&self) -> &Asset<DocumentData> {
        self.data
    }

    pub fn index(&self) -> usize {
        self.index
    }
}

/// References a node in a gltf document
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GltfNodeRef<'a> {
    data: &'a Asset<DocumentData>,
    index: usize,
}

impl<'a> GltfNodeRef<'a> {
    pub fn name(&self) -> Option<&str> {
        self.data.node(self.index).and_then(|v| v.name())
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn data(&self) -> &Asset<DocumentData> {
        self.data
    }

    pub fn mesh(&'a self) -> Option<GltfMeshRef<'a>> {
        let node = self.data.node(self.index).unwrap();
        Some(GltfMeshRef::from_gltf(self.data, node.mesh()?))
    }

    pub fn children(&'a self) -> impl Iterator<Item = GltfNodeRef<'a>> {
        self.data
            .node(self.index)
            .unwrap()
            .children()
            .map(move |v| GltfNodeRef {
                data: self.data,
                index: v.index(),
            })
    }
}

/// References a mesh primitive in a gltf document
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GltfPrimitive {
    data: Asset<DocumentData>,
    index: usize,
}

// TODO: macro for all these
impl GltfPrimitive {
    pub fn index(&self) -> usize {
        self.index
    }

    pub fn data(&self) -> &Asset<DocumentData> {
        &self.data
    }

    pub fn material(&self) -> GltfMaterialRef {
        GltfMaterialRef {
            data: &self.data,
            index: self
                .data
                .primitive(self.index())
                .unwrap()
                .material()
                .index()
                .unwrap(),
        }
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

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn data(&self) -> &Asset<DocumentData> {
        &self.data
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

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn data(&self) -> &Asset<DocumentData> {
        &self.data
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

    pub fn data(&self) -> &DocumentData {
        &self.data
    }

    pub fn index(&self) -> usize {
        self.index
    }

    pub fn get_ref(&self) -> GltfNodeRef {
        GltfNodeRef {
            data: &self.data,
            index: self.index,
        }
    }

    pub fn mesh(&self) -> Option<GltfMeshRef<'_>> {
        let node = self.data.node(self.index).unwrap();
        Some(GltfMeshRef::from_gltf(&self.data, node.mesh()?))
    }

    pub fn children(&self) -> impl Iterator<Item = GltfNodeRef> {
        self.data
            .node(self.index)
            .unwrap()
            .children()
            .map(move |v| GltfNodeRef {
                data: &self.data,
                index: v.index(),
            })
    }
}

impl From<GltfMeshRef<'_>> for GltfMesh {
    fn from(v: GltfMeshRef) -> Self {
        Self {
            data: v.data.clone(),
            index: v.index,
        }
    }
}

impl From<GltfMaterialRef<'_>> for GltfMaterial {
    fn from(v: GltfMaterialRef) -> Self {
        Self {
            data: v.data.clone(),
            index: v.index,
        }
    }
}

impl From<GltfNodeRef<'_>> for GltfNode {
    fn from(v: GltfNodeRef) -> Self {
        Self {
            data: v.data.clone(),
            index: v.index,
        }
    }
}
