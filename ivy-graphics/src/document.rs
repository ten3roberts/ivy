use crate::{Error, Material, Mesh, PointLight, Result};
use hecs::{Bundle, Component, EntityBuilder, EntityBuilderClone};
use std::{borrow::Cow, path::Path, path::PathBuf, sync::Arc};

use ivy_base::{Position, Rotation, Scale, Visible};
use ivy_resources::{Handle, LoadResource, Resources};
use ivy_vulkan::{Texture, VulkanContext};
use ultraviolet::*;

#[derive(Debug, Clone)]
pub struct Node {
    /// The name of this node.
    name: String,
    /// The mesh index references by this node.
    mesh: Option<usize>,
    light: Option<PointLight>,
    pos: Position,
    rot: Rotation,
    scale: Scale,
}

impl Node {
    /// Get a reference to the node's mesh.
    pub fn mesh(&self) -> Option<usize> {
        self.mesh
    }

    /// Get a reference to the node's light.
    pub fn light(&self) -> Option<PointLight> {
        self.light
    }

    /// Get a reference to the node's position.
    pub fn pos(&self) -> Position {
        self.pos
    }

    /// Get a reference to the node's rotation.
    pub fn rot(&self) -> Rotation {
        self.rot
    }

    /// Get a reference to the node's scale.
    pub fn scale(&self) -> Scale {
        self.scale
    }

    /// Get a reference to the node's name.
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }
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
        let (document, buffers, _images) = {
            gltf::import(&path).map_err(|e| Error::GltfImport(e, Some(path.to_owned().into())))
        }?;

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
        let mut texture_cache = resources.fetch_mut::<Texture>()?;

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

        let mut material_cache = resources.fetch_mut::<Material>()?;
        let materials = document
            .materials()
            .map(|material| {
                Material::from_gltf(context.clone(), material, &textures, resources)
                    .map(|material| material_cache.insert(material))
            })
            .collect::<Result<Vec<_>>>()?;

        drop(material_cache);

        let mut mesh_cache = resources.fetch_mut::<Mesh>()?;
        let meshes = document
            .meshes()
            .map(|mesh| {
                Mesh::from_gltf(context.clone(), mesh, buffers, &materials)
                    .map(|mesh| mesh_cache.insert(mesh))
            })
            .collect::<Result<Vec<_>>>()?;

        drop(mesh_cache);

        let nodes = document
            .nodes()
            .map(|node| {
                let (position, rotation, scale) = node.transform().decomposed();
                let light = node
                    .light()
                    .map(|val| PointLight::new(0.1, Vec3::from(val.color()) * val.intensity()));

                Node {
                    name: node.name().unwrap_or_default().to_owned(),
                    light,
                    mesh: node.mesh().map(|mesh| mesh.index()),
                    pos: Vec3::from(position).into(),
                    rot: Rotor3::from_quaternion_array(rotation).into(),
                    scale: Vec3::from(scale).into(),
                }
            })
            .collect();

        Ok(Self {
            meshes,
            nodes,
            materials,
        })
    }
}

impl Document {
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

    /// Returns the nodes in the document
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Returns a reference to the node at index.
    pub fn node(&self, index: usize) -> &Node {
        &self.nodes[index]
    }

    /// Searches for the node with name.
    pub fn find_node<S>(&self, name: S) -> Result<&Node>
    where
        S: AsRef<str>,
    {
        let name = name.as_ref();
        self.nodes
            .iter()
            .find(|node| node.name == name)
            .ok_or_else(|| Error::UnknownDocumentNode(name.to_owned()))
    }

    pub fn build_node_by_name<'a, S: AsRef<str>, B: GenericBuilder>(
        &self,
        name: S,
        builder: &'a mut B,
    ) -> Result<&'a mut B> {
        let node = self.find_node(name)?;
        Ok(self.build_node_internal(node, builder))
    }

    /// Spawns a node using the supplied builder into the world
    pub fn build_node<'a, B: GenericBuilder>(&self, index: usize, builder: &'a mut B) -> &'a mut B {
        self.build_node_internal(self.node(index), builder)
    }
    /// Spawns a node using the supplied builder into the world
    fn build_node_internal<'a, B: GenericBuilder>(
        &self,
        node: &Node,
        builder: &'a mut B,
    ) -> &'a mut B {
        if let Some(mesh) = node.mesh {
            builder.add::<Handle<Mesh>>(self.mesh(mesh));
        }

        if let Some(light) = node.light {
            eprintln!("Building light");
            builder.add(light);
        }

        builder.add(node.pos);
        builder.add(node.rot);
        builder.add(node.scale);
        builder.add(Visible::default());

        builder
    }
}

#[derive(Bundle, Copy, Clone, Default)]
pub struct NodeBundle {
    pos: Position,
    rot: Rotation,
    scale: Scale,
    mesh: Handle<Mesh>,
}

impl LoadResource for Document {
    type Info = Cow<'static, str>;

    type Error = Error;

    fn load(resources: &ivy_resources::Resources, path: &Self::Info) -> Result<Self> {
        let context = resources.get_default::<Arc<VulkanContext>>()?;
        Self::from_file(context.clone(), resources, path.as_ref())
    }
}

// Generic interface for cloneable and non coneable entity builders.
pub trait GenericBuilder {
    fn add<T: Component + Clone>(&mut self, component: T) -> &mut Self;
}

impl GenericBuilder for EntityBuilder {
    fn add<T: Component + Clone>(&mut self, component: T) -> &mut Self {
        self.add(component)
    }
}

impl GenericBuilder for EntityBuilderClone {
    fn add<T: Component + Clone>(&mut self, component: T) -> &mut Self {
        self.add(component)
    }
}
