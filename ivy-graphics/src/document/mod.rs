use crate::{
    Animation, AnimationStore, Animator, Error, Material, Mesh, PointLight, Result, SkinnedMesh,
};
use gltf::Gltf;
use hecs::{Bundle, Component, DynamicBundleClone, EntityBuilder, EntityBuilderClone};
use hecs_hierarchy::TreeBuilderClone;
use smallvec::SmallVec;
use std::{borrow::Cow, ops::Deref, path::Path, path::PathBuf};

use glam::*;
use ivy_base::{Connection, Position, PositionOffset, Rotation, RotationOffset, Scale, Visible};
use ivy_resources::{Handle, LoadResource, Resources};
use ivy_vulkan::{context::SharedVulkanContext, Texture};

mod joint;
pub(crate) mod scheme;
pub(crate) mod util;
pub use joint::*;

pub(crate) use scheme::*;
pub(crate) use util::*;

#[derive(Debug, Clone)]
#[doc(hidden)]
pub struct Node {
    /// The name of this node.
    name: String,
    /// The mesh index references by this node.
    mesh: Option<Handle<Mesh>>,
    skinned_mesh: Option<Handle<SkinnedMesh>>,
    animations: AnimationStore,
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

    /// Get a reference to the node's animations.
    pub fn animations(&self) -> &AnimationStore {
        &self.animations
    }
}

pub struct Document {
    meshes: Vec<Handle<Mesh>>,
    materials: Vec<Handle<Material>>,
    skins: Vec<Handle<Skin>>,
    nodes: Vec<Node>,
}

impl Document {
    /// Loads a gltf document/asset from path
    pub fn from_file<P, O>(
        context: SharedVulkanContext,
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
        context: SharedVulkanContext,
        resources: &Resources,
        document: gltf::Document,
        buffers: &[gltf::buffer::Data],
        textures: Vec<Handle<Texture>>,
    ) -> Result<Self> {
        let mut material_cache = resources.fetch_mut::<Material>()?;
        let materials = document
            .materials()
            .map(|material| {
                Material::from_gltf(&context, material, &textures, resources)
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

        let skins: Vec<_> = document
            .skins()
            .map(|skin| -> Result<_> { Skin::from_gltf(&document, skin, buffers) })
            .collect::<Result<Vec<_>>>()?;

        let mut animations = resources.fetch_mut::<Animation>()?;

        let animations = document
            .animations()
            .flat_map(|anim| Animation::from_gltf(anim, &skins, buffers))
            .map(|val| (val.skin(), (val.name().to_string(), animations.insert(val))))
            .collect::<Vec<_>>();

        let mut skin_cache = resources.fetch_mut()?;
        let skins: Vec<_> = skins
            .into_iter()
            .map(|val| skin_cache.insert(val))
            .collect();

        drop(mesh_cache);

        let nodes = document
            .nodes()
            .map(|node| {
                let (position, rotation, scale) = node.transform().decomposed();
                let light = node
                    .light()
                    .map(|val| PointLight::new(0.1, Vec3::from(val.color()) * val.intensity()));

                let skin = node.skin().map(|skin| skins[skin.index()]);
                let animations = if let Some(skin) = node.skin() {
                    let skin = skin.index();
                    let animations = animations.iter().filter_map(|val| {
                        if val.0 == skin {
                            Some(val.1.clone())
                        } else {
                            None
                        }
                    });

                    AnimationStore::from(animations)
                } else {
                    AnimationStore::new()
                };

                Node {
                    name: node.name().unwrap_or_default().to_owned(),
                    skin,
                    animations,
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
            skins,
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

    /// Returns the skin at index
    pub fn skin(&self, index: usize) -> Handle<Skin> {
        self.skins[index]
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

    /// Returns a reference to the node at index.
    pub fn node(&self, index: usize) -> DocumentNode {
        let node = &self.nodes[index];
        DocumentNode {
            node,
            index,
            document: self,
        }
    }

    /// Searches for the node with name.
    pub fn find<S>(&self, name: S) -> Result<DocumentNode>
    where
        S: AsRef<str>,
    {
        let name = name.as_ref();
        let index = self
            .nodes
            .iter()
            .position(|node| node.name == name)
            .ok_or_else(|| Error::UnknownDocumentNode(name.to_owned()))?;

        Ok(self.node(index))
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
        let context = resources.get_default::<SharedVulkanContext>()?;
        Self::from_file(context.clone(), resources, path.as_ref())
    }
}

// Generic interface for cloneable and non coneable entity builders.
pub trait GenericBuilder {
    fn add<T: Component + Clone>(&mut self, component: T) -> &mut Self;
    fn add_bundle<T: DynamicBundleClone>(&mut self, bundle: T) -> &mut Self;
}

impl GenericBuilder for EntityBuilder {
    fn add<T: Component + Clone>(&mut self, component: T) -> &mut Self {
        self.add(component)
    }

    fn add_bundle<T: DynamicBundleClone>(&mut self, bundle: T) -> &mut Self {
        self.add_bundle(bundle)
    }
}

impl GenericBuilder for EntityBuilderClone {
    fn add<T: Component + Clone>(&mut self, component: T) -> &mut Self {
        self.add(component)
    }

    fn add_bundle<T: DynamicBundleClone>(&mut self, bundle: T) -> &mut Self {
        self.add_bundle(bundle)
    }
}

#[records::record]
#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct NodeBuildInfo {
    skinned: bool,
    /// Insert an animator
    animated: bool,
    light_radius: f32,
}

#[derive(Clone)]
pub struct DocumentNode<'a> {
    node: &'a Node,
    index: usize,
    document: &'a Document,
}

impl<'a> Deref for DocumentNode<'a> {
    type Target = Node;

    fn deref(&self) -> &Self::Target {
        self.node
    }
}

impl<'a> std::fmt::Debug for DocumentNode<'a> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DocumentNode")
            .field("node", &self.node)
            .field("index", &self.index)
            .finish()
    }
}

impl<'d> DocumentNode<'d> {
    /// Spawns a node using the supplied builder into the world
    pub fn build<'a, B: GenericBuilder>(
        &self,
        builder: &'a mut B,
        info: &NodeBuildInfo,
    ) -> &'a mut B {
        if let Some(mesh) = self.mesh {
            builder.add(mesh);
        }

        if let Some(mut light) = self.light {
            light.radius = info.light_radius;
            builder.add(light);
        }

        // Add skinning info
        if info.skinned {
            if let Some(mesh) = self.skinned_mesh {
                builder.add(mesh);
            }
            if let Some(skin) = self.skin {
                builder.add(skin);
            }
        }

        if info.animated {
            builder.add(Animator::new(self.animations.clone()));
        }

        builder.add_bundle((
            self.pos,
            self.rot,
            self.scale,
            PositionOffset(*self.pos),
            RotationOffset(*self.rot),
            Visible::default(),
        ));

        builder
    }

    /// Recursively build the whole tree with node as root.
    ///
    /// Requires access to the orignal document.
    ///
    /// Behavior is undefined if a different document is used
    pub fn build_tree<'a>(
        &self,
        builder: &'a mut TreeBuilderClone<Connection>,
        info: &NodeBuildInfo,
    ) -> &'a mut TreeBuilderClone<Connection> {
        self.build(builder.root_mut(), info);

        for child in self.children() {
            let mut subtree = TreeBuilderClone::new();
            let child = self.document.node(*child);
            child.build_tree(&mut subtree, info);
            builder.attach(subtree);
        }

        builder
    }

    /// Get the document node's index.
    pub fn index(&self) -> usize {
        self.index
    }

    /// Get the original document
    pub fn document(&self) -> &Document {
        self.document
    }

    /// Get the document node's node.
    pub fn node(&self) -> &Node {
        self.node
    }
}
