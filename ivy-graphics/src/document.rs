use crate::{Error, Material, Mesh, Result};
use std::{borrow::Cow, path::Path, path::PathBuf, sync::Arc};

use ivy_resources::{Handle, LoadResource, Resources};
use ivy_vulkan::{Texture, VulkanContext};
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
    materials: Vec<Handle<Material>>,
    nodes: Vec<Node>,
}

impl Document {
    /// Loads a gltf document/asset from path
    pub fn from_file<P, O>(
        context: Arc<VulkanContext>,
        resources: &Resources,
        path: P,
    ) -> Result<Self>
    where
        P: AsRef<Path> + ToOwned<Owned = O>,
        O: Into<PathBuf>,
    {
        let (document, buffers, _images) =
            gltf::import(&path).map_err(|e| Error::GltfImport(e, Some(path.to_owned().into())))?;

        Self::from_gltf(context, resources, document, &buffers)
    }

    /// Loads a gltf import document's meshes and scene data. Will insert the meshes into the
    /// provided resource cache.
    pub fn from_gltf(
        context: Arc<VulkanContext>,
        resources: &Resources,
        document: gltf::Document,
        buffers: &[gltf::buffer::Data],
    ) -> Result<Self> {
        let mut mesh_cache = resources.fetch_mut::<Mesh>()?;
        let mut material_cache = resources.fetch_mut::<Material>()?;
        let mut texture_cache = resources.fetch_mut::<Texture>()?;

        let meshes = document
            .meshes()
            .map(|mesh| {
                Mesh::from_gltf(context.clone(), mesh, buffers).map(|mesh| mesh_cache.insert(mesh))
            })
            .collect::<Result<Vec<_>>>()?;

        drop(mesh_cache);

        let textures = document
            .textures()
            .map(|val| {
                let data = val.source().source();
                let texture = match data {
                    gltf::image::Source::View { view, mime_type: _ } => {
                        let buffer = &buffers[view.buffer().index()];
                        let raw = &buffer[view.offset()..view.offset() + view.length()];
                        let texture = Texture::from_memory(context.clone(), raw)?;
                        Ok(texture)
                    }
                    gltf::image::Source::Uri { uri, mime_type: _ } => {
                        Texture::load(context.clone(), uri)
                    }
                };
                texture
                    .map(|val| texture_cache.insert(val))
                    .map_err(|e| e.into())
            })
            .collect::<Result<Vec<_>>>()?;

        drop(texture_cache);

        let materials = document
            .materials()
            .map(|material| {
                Material::from_gltf(context.clone(), material, &textures, resources)
                    .map(|material| material_cache.insert(material))
            })
            .collect::<Result<Vec<_>>>()?;

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

        Ok(Self {
            meshes,
            nodes,
            materials,
        })
    }

    /// Returns a handle to the mesh at index. Mesh was inserted in the resource cache upon
    /// creation.
    pub fn material(&self, index: usize) -> Handle<Material> {
        self.materials[index]
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

impl LoadResource for Document {
    type Info = Cow<'static, str>;

    type Error = Error;

    fn load(resources: &ivy_resources::Resources, path: &Self::Info) -> Result<Self> {
        let context = resources.get_default::<Arc<VulkanContext>>()?;
        Self::from_file(context.clone(), resources, path.as_ref())
    }
}
