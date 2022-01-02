use crate::{Animation, Error, Material, Mesh, PointLight, Result, SkinnedMesh};
use gltf::Gltf;
use hecs::{Bundle, Component, EntityBuilder, EntityBuilderClone};
use smallvec::SmallVec;
use std::{borrow::Cow, path::Path, path::PathBuf, sync::Arc};

use glam::*;
use ivy_base::{Position, Rotation, Scale, Visible};
use ivy_resources::{Handle, LoadResource, Resources};
use ivy_vulkan::{Texture, VulkanContext};

mod joint;
pub(crate) mod scheme;
pub(crate) mod util;
pub use joint::*;

pub(crate) use scheme::*;
pub(crate) use util::*;

#[derive(Debug, Clone)]
pub struct Node {
    /// The name of this node.
    name: String,
    /// The mesh index references by this node.
    mesh: Option<Handle<Mesh>>,
    skinned_mesh: Option<Handle<SkinnedMesh>>,
    light: Option<PointLight>,
    skin: Option<Handle<Skin>>,
    pos: Position,
    rot: Rotation,
    scale: Scale,
    children: SmallVec<[usize; 4]>,
}

impl Node {
    /// Get a reference to the node's mesh.
    pub fn mesh(&self) -> Option<Handle<Mesh>> {
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

    /// Get a reference to the node's skin.
    pub fn skin(&self) -> Option<Handle<Skin>> {
        self.skin
    }

    /// Get a reference to the node's children.
    pub fn children(&self) -> &SmallVec<[usize; 4]> {
        &self.children
    }

    /// Get a reference to the node's skinned mesh.
    pub fn skinned_mesh(&self) -> Option<Handle<SkinnedMesh>> {
        self.skinned_mesh
    }
}

pub struct Document {
    meshes: Vec<Handle<Mesh>>,
    materials: Vec<Handle<Material>>,
    animations: Vec<Handle<Animation>>,
    skins: Vec<Handle<Skin>>,
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
        let path = path.as_ref();
        let Gltf { document, blob } =
            Gltf::open(path).map_err(|e| Error::GltfImport(e, Some(path.to_owned().into())))?;

        let buffers = import_buffer_data(&document, blob, path)?;

        let textures = import_image_data(&document, path, &buffers, resources)?;

        Self::from_gltf(context, resources, document, &buffers, textures)
    }

    /// Loads a gltf import document's meshes and scene data. Will insert the meshes into the
    /// provided resource cache.
    pub fn from_gltf(
        context: Arc<VulkanContext>,
        resources: &Resources,
        document: gltf::Document,
        buffers: &[gltf::buffer::Data],
        textures: Vec<Handle<Texture>>,
    ) -> Result<Self> {
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

        let mut skinned_meshes = resources.fetch_mut::<SkinnedMesh>()?;

        // There may not be joint and weight information of all meshes
        let skinned_meshes = document
            .meshes()
            .map(|mesh| -> Result<Option<Handle<SkinnedMesh>>> {
                match Mesh::from_gltf_skinned(context.clone(), mesh, buffers, &materials) {
                    Ok(val) => Ok(Some(skinned_meshes.insert(val))),
                    Err(Error::EmptyMesh) => Ok(None),
                    Err(e) => Err(e),
                }
            })
            .collect::<Result<Vec<_>>>()?;
        let mut animations = resources.fetch_mut::<Animation>()?;

        let animations = document
            .animations()
            .map(|anim| Animation::from_gltf(anim, buffers))
            .map(|val| animations.insert(val))
            .collect::<Vec<_>>();

        let mut skins = resources.fetch_mut()?;

        let skins: Vec<_> = document
            .skins()
            .map(|skin| Skin::from_gltf(&document, skin, buffers).map(|skin| skins.insert(skin)))
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
                    skin: node.skin().map(|skin| skins[skin.index()]),
                    light,
                    mesh: node.mesh().map(|mesh| meshes[mesh.index()]),
                    skinned_mesh: node.mesh().and_then(|mesh| skinned_meshes[mesh.index()]),
                    pos: Vec3::from(position).into(),
                    rot: Quat::from_array(rotation).into(),
                    scale: Vec3::from(scale).into(),
                    children: node.children().map(|val| val.index()).collect(),
                }
            })
            .collect();

        Ok(Self {
            meshes,
            animations,
            skins,
            nodes,
            materials,
        })
    }

    /// Get a reference to the document's animations.
    pub fn animations(&self) -> &[Handle<Animation>] {
        self.animations.as_ref()
    }
}

impl Document {
    /// Returns a handle to the mesh at index. Mesh was inserted in the resource cache upon
    /// creation.
    pub fn material(&self, index: usize) -> Handle<Material> {
        self.materials[index]
    }

    /// Returns the skin at index
    pub fn skin(&self, index: usize) -> Handle<Skin> {
        self.skins[index]
    }

    /// Returns the animation at index
    pub fn animation(&self, index: usize) -> Handle<Animation> {
        self.animations[index]
    }

    /// Get a reference to the document's skins.
    pub fn skins(&self) -> &[Handle<Skin>] {
        self.skins.as_ref()
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
        info: &NodeBuildInfo,
    ) -> Result<&'a mut B> {
        let node = self.find_node(name)?;
        Ok(self.build_node(node, builder, info))
    }

    /// Spawns a node using the supplied builder into the world
    pub fn build_node_by_index<'a, B: GenericBuilder>(
        &self,
        index: usize,
        builder: &'a mut B,
        info: &NodeBuildInfo,
    ) -> &'a mut B {
        self.build_node(self.node(index), builder, info)
    }
    /// Spawns a node using the supplied builder into the world
    pub fn build_node<'a, B: GenericBuilder>(
        &self,
        node: &Node,
        builder: &'a mut B,
        info: &NodeBuildInfo,
    ) -> &'a mut B {
        if let Some(mesh) = node.mesh {
            builder.add(mesh);
        }

        if let Some(light) = node.light {
            builder.add(light);
        }

        // Add skinning info
        if info.skinned {
            if let Some(mesh) = node.skinned_mesh {
                builder.add(mesh);
            }
            if let Some(skin) = node.skin {
                builder.add(skin);
            }
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

#[records::record]
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct NodeBuildInfo {
    skinned: bool,
}
