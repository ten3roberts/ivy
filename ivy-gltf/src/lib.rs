use itertools::Itertools;
use std::{collections::HashMap, path::Path};

use gltf::Gltf;
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
            name: v.name(),
        })
    }

    pub fn materials(&self) -> impl Iterator<Item = GltfMaterialRef> {
        self.data
            .materials()
            .enumerate()
            .map(|(i, v)| GltfMaterialRef {
                data: &self.data,
                index: i,
                name: v.name(),
            })
    }

    pub fn nodes(&self) -> impl Iterator<Item = GltfNodeRef> {
        self.data.nodes().enumerate().map(|(i, v)| GltfNodeRef {
            data: &self.data,
            index: i,
            name: v.name(),
        })
    }

    pub fn data(&self) -> &DocumentData {
        &self.data
    }

    pub fn mesh(&self, index: usize) -> Option<GltfMeshRef> {
        self.data.meshes().nth(index).map(|v| GltfMeshRef {
            data: &self.data,
            index,
            name: v.name(),
        })
    }

    pub fn material(&self, index: usize) -> Option<GltfMaterialRef> {
        self.data.materials().nth(index).map(|v| GltfMaterialRef {
            data: &self.data,
            index,
            name: v.name(),
        })
    }

    pub fn node(&self, index: usize) -> Option<GltfNodeRef> {
        self.data.nodes().nth(index).map(|v| GltfNodeRef {
            data: &self.data,
            index,
            name: v.name(),
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

/// References a mesh in a gltf document
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GltfMeshRef<'a> {
    data: &'a Asset<DocumentData>,
    index: usize,
    name: Option<&'a str>,
}

impl<'a> GltfMeshRef<'a> {
    pub fn data(&self) -> &Asset<DocumentData> {
        self.data
    }

    pub fn name(&self) -> Option<&str> {
        self.name
    }
}

/// References a material in a gltf document
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GltfMaterialRef<'a> {
    data: &'a Asset<DocumentData>,
    index: usize,
    name: Option<&'a str>,
}

impl<'a> GltfMaterialRef<'a> {
    pub fn name(&self) -> Option<&str> {
        self.name
    }

    pub fn data(&self) -> &Asset<DocumentData> {
        self.data
    }
}

/// References a node in a gltf document
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GltfNodeRef<'a> {
    data: &'a Asset<DocumentData>,
    index: usize,
    name: Option<&'a str>,
}

impl<'a> GltfNodeRef<'a> {
    pub fn index(&self) -> usize {
        self.index
    }

    pub fn name(&self) -> Option<&str> {
        self.name
    }

    pub fn data(&self) -> &Asset<DocumentData> {
        self.data
    }
}

/// References a mesh in a gltf document
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GltfMesh {
    data: Asset<DocumentData>,
    index: usize,
}

impl GltfMesh {
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
