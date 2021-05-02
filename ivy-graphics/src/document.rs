use std::{path::Path, rc::Rc, sync::Arc};

use crate::Error;

use super::Mesh;
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
    meshes: Vec<Arc<Mesh>>,
    nodes: Vec<Node>,
}

impl Document {
    /// Loads a gltf document/asset from path
    pub fn load<P: AsRef<Path>>(context: Rc<VulkanContext>, path: P) -> Result<Self, Error> {
        let (document, buffers, _images) = gltf::import(path)?;

        Self::from_gltf(context, document, &buffers)
    }

    /// Loads a gltf import document's meshes and scene data.
    pub fn from_gltf(
        context: Rc<VulkanContext>,
        document: gltf::Document,
        buffers: &[gltf::buffer::Data],
    ) -> Result<Self, Error> {
        let meshes = document
            .meshes()
            .map(|mesh| Mesh::from_gltf(context.clone(), mesh, buffers).map(Arc::new))
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

        Ok(Self { nodes, meshes })
    }

    /// Returns a handle to the mesh at index.
    pub fn mesh(&self, index: usize) -> &Arc<Mesh> {
        &self.meshes[index]
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
