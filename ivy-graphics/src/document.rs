use std::{path::Path, path::PathBuf, sync::Arc};

use crate::Error;

use super::Mesh;
use ivy_resources::{Handle, ResourceCache};
use ivy_vulkan::VulkanContext;
use ultraviolet::*;

#[derive(Debug, Clone)]
pub struct Node {
    /// The name of this node.
    name: String,
    /// The mesh index references by this node.
    mesh: Option<usize>,
    position: Vec3,
    rotation: Rotor3,
    scale: Vec3,
}

pub struct Document {
    meshes: Vec<Handle<Mesh>>,
    nodes: Vec<Node>,
}

impl Document {
    /// Loads a gltf document/asset from path
    pub fn load<P, O>(
        context: Arc<VulkanContext>,
        meshes: &mut ResourceCache<Mesh>,
        path: P,
    ) -> Result<Self, Error>
    where
        P: AsRef<Path> + ToOwned<Owned = O>,
        O: Into<PathBuf>,
    {
        let (document, buffers, _images) =
            gltf::import(&path).map_err(|e| Error::GltfImport(e, Some(path.to_owned().into())))?;

        Self::from_gltf(context, meshes, document, &buffers)
    }

    /// Loads a gltf import document's meshes and scene data. Will insert the meshes into the
    /// provided resource cache.
    pub fn from_gltf(
        context: Arc<VulkanContext>,
        meshes: &mut ResourceCache<Mesh>,
        document: gltf::Document,
        buffers: &[gltf::buffer::Data],
    ) -> Result<Self, Error> {
        let meshes = document
            .meshes()
            .map(|mesh| {
                Mesh::from_gltf(context.clone(), mesh, buffers).map(|mesh| meshes.insert(mesh))
            })
            .collect::<Result<Vec<_>, _>>()?;

        let nodes = document
            .nodes()
            .map(|node| {
                let (position, rotation, scale) = node.transform().decomposed();
                Node {
                    name: node.name().unwrap_or_default().to_owned(),
                    mesh: node.mesh().map(|mesh| mesh.index()),
                    position: Vec3::from(position),
                    rotation: Rotor3::from_quaternion_array(rotation),
                    scale: Vec3::from(scale),
                }
            })
            .collect();

        Ok(Self { meshes, nodes })
    }

    /// Returns a handle to the mesh at index. Mesh was inserted in the resource cache upon
    /// creation.
    pub fn mesh(&self, index: usize) -> Handle<Mesh> {
        self.meshes[index]
    }

    /// Returns a reference to the node at index.
    pub fn node(&self, index: usize) -> &Node {
        &self.nodes[index]
    }

    /// Searches for the node with name.
    pub fn find_node<S>(&self, name: S) -> Option<&Node>
    where
        S: AsRef<str>,
    {
        let name = name.as_ref();
        self.nodes.iter().find(|node| node.name == name)
    }
}
