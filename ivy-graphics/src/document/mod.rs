use crate::{
    components, Animation, AnimationStore, Animator, Error, Material, Mesh, PointLight, Result,
    SkinnedMesh,
};
use flax::{components::child_of, EntityBuilder};
use gltf::Gltf;
use ivy_assets::{Asset, AssetCache, AssetKey};
use smallvec::SmallVec;
use std::{ops::Deref, path::Path, path::PathBuf};

use glam::*;
use ivy_base::{position, position_offset, rotation, rotation_offset, scale, visible, Visible};
use ivy_vulkan::{
    context::{SharedVulkanContext, VulkanContextService},
    Texture,
};

mod joint;
pub(crate) mod scheme;
pub(crate) mod util;
pub use joint::*;

pub(crate) use scheme::*;
pub(crate) use util::*;

#[derive(Clone)]
#[doc(hidden)]
pub struct Node {
    /// The name of this node.
    name: String,
    /// The mesh index references by this node.
    mesh: Option<Asset<Mesh>>,
    skinned_mesh: Option<Asset<SkinnedMesh>>,
    animations: AnimationStore,
    light: Option<PointLight>,
    skin: Option<Asset<Skin>>,
    pos: Vec3,
    rot: Quat,
    scale: Vec3,
    children: SmallVec<[usize; 4]>,
}

impl Node {
    /// Get a reference to the node's mesh.
    pub fn mesh(&self) -> Option<&Asset<Mesh>> {
        self.mesh.as_ref()
    }

    /// Get a reference to the node's light.
    pub fn light(&self) -> Option<PointLight> {
        self.light
    }

    /// Get a reference to the node's position.
    pub fn pos(&self) -> Vec3 {
        self.pos
    }

    /// Get a reference to the node's rotation.
    pub fn rot(&self) -> Quat {
        self.rot
    }

    /// Get a reference to the node's scale.
    pub fn scale(&self) -> Vec3 {
        self.scale
    }

    /// Get a reference to the node's name.
    pub fn name(&self) -> &str {
        self.name.as_ref()
    }

    /// Get a reference to the node's skin.
    pub fn skin(&self) -> Option<&Asset<Skin>> {
        self.skin.as_ref()
    }

    /// Get a reference to the node's children.
    pub fn children(&self) -> &SmallVec<[usize; 4]> {
        &self.children
    }

    /// Get a reference to the node's skinned mesh.
    pub fn skinned_mesh(&self) -> Option<&Asset<SkinnedMesh>> {
        self.skinned_mesh.as_ref()
    }

    /// Get a reference to the node's animations.
    pub fn animations(&self) -> &AnimationStore {
        &self.animations
    }
}

pub struct Document {
    meshes: Vec<Asset<Mesh>>,
    materials: Vec<Asset<Material>>,
    skins: Vec<Asset<Skin>>,
    nodes: Vec<Node>,
}

impl Document {
    /// Loads a gltf document/asset from path
    pub fn from_file<P, O>(
        context: SharedVulkanContext,
        assets: &AssetCache,
        path: P,
    ) -> Result<Self>
    where
        P: AsRef<Path> + ToOwned<Owned = O>,
        O: Into<PathBuf>,
    {
        let path = path.as_ref();
        let Gltf { document, blob } =
            Gltf::open(path).map_err(|e| Error::GltfImport(e, Some(path.to_owned())))?;

        let buffers = import_buffer_data(&document, blob, path)?;

        let textures = import_image_data(assets, &document, path, &buffers)?;

        Self::from_gltf(context, assets, document, &buffers, textures)
    }

    /// Loads a gltf import document's meshes and scene data. Will insert the meshes into the
    /// provided resource cache.
    pub fn from_gltf(
        context: SharedVulkanContext,
        assets: &AssetCache,
        document: gltf::Document,
        buffers: &[gltf::buffer::Data],
        textures: Vec<Asset<Texture>>,
    ) -> Result<Self> {
        let materials = document
            .materials()
            .map(|material| Ok(assets.insert(Material::from_gltf(assets, material, &textures)?)))
            .collect::<Result<Vec<_>>>()?;

        let meshes = document
            .meshes()
            .map(|mesh| {
                Mesh::from_gltf(context.clone(), mesh, buffers, &materials)
                    .map(|mesh| assets.insert(mesh))
            })
            .collect::<Result<Vec<_>>>()?;

        // There may not be joint and weight information of all meshes
        let skinned_meshes = document
            .meshes()
            .map(|mesh| -> Result<Option<Asset<SkinnedMesh>>> {
                match Mesh::from_gltf_skinned(context.clone(), mesh, buffers, &materials) {
                    Ok(val) => Ok(Some(assets.insert(val))),
                    Err(Error::EmptyMesh) => Ok(None),
                    Err(e) => Err(e),
                }
            })
            .collect::<Result<Vec<_>>>()?;

        let skins: Vec<_> = document
            .skins()
            .map(|skin| -> Result<_> { Skin::from_gltf(&document, skin, buffers) })
            .collect::<Result<Vec<_>>>()?;

        let animations = document
            .animations()
            .flat_map(|anim| Animation::from_gltf(anim, &skins, buffers))
            .map(|val| (val.skin(), (val.name().to_string(), assets.insert(val))))
            .collect::<Vec<_>>();

        let skins: Vec<_> = skins.into_iter().map(|val| assets.insert(val)).collect();

        let nodes = document
            .nodes()
            .map(|node| {
                let (position, rotation, scale) = node.transform().decomposed();
                let light = node
                    .light()
                    .map(|val| PointLight::new(0.1, Vec3::from(val.color()) * val.intensity()));

                let skin = node.skin().map(|skin| skins[skin.index()].clone());
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
                    mesh: node.mesh().map(|mesh| meshes[mesh.index()].clone()),
                    skinned_mesh: node
                        .mesh()
                        .and_then(|mesh| skinned_meshes[mesh.index()].clone()),
                    pos: Vec3::from(position),
                    rot: Quat::from_array(rotation),
                    scale: Vec3::from(scale),
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
    pub fn material(&self, index: usize) -> &Asset<Material> {
        &self.materials[index]
    }

    /// Returns the skin at index
    pub fn skin(&self, index: usize) -> &Asset<Skin> {
        &self.skins[index]
    }

    /// Get a reference to the document's skins.
    pub fn skins(&self) -> &[Asset<Skin>] {
        self.skins.as_ref()
    }

    /// Returns a handle to the mesh at index. Mesh was inserted in the resource cache upon
    /// creation.
    pub fn mesh(&self, index: usize) -> &Asset<Mesh> {
        &self.meshes[index]
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DocumentFromPath(pub PathBuf);

impl AssetKey<Document> for DocumentFromPath {
    type Error = Error;

    fn load(&self, assets: &AssetCache) -> Result<Asset<Document>> {
        Ok(assets.insert(Document::from_file(
            assets.service::<VulkanContextService>().context(),
            assets,
            &self.0,
        )?))
    }
}

#[derive(Default, Debug, Clone, Copy, PartialEq)]
pub struct NodeBuildInfo {
    pub skinned: bool,
    /// Insert an animator
    pub animated: bool,
    pub light_radius: f32,
}

impl NodeBuildInfo {
    pub fn new(skinned: bool, animated: bool, light_radius: f32) -> Self {
        Self {
            skinned,
            animated,
            light_radius,
        }
    }
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
            // .field("node", &self.node)
            .field("index", &self.index)
            .finish()
    }
}

impl<'d> DocumentNode<'d> {
    /// Spawns a node using the supplied builder into the world
    pub fn mount<'a>(
        &self,
        entity: &'a mut EntityBuilder,
        info: &NodeBuildInfo,
    ) -> &'a mut EntityBuilder {
        if let Some(mesh) = self.mesh.clone() {
            entity.set(components::mesh(), mesh);
        }

        if let Some(mut light) = self.light {
            light.radius = info.light_radius;
            entity.set(components::light_source(), light);
        }

        // set skinning info
        if info.skinned {
            if let Some(mesh) = self.skinned_mesh.clone() {
                entity.set(components::skinned_mesh(), mesh);
            }
            if let Some(skin) = self.skin.clone() {
                entity.set(components::skin(), skin);
            }
        }

        if info.animated {
            entity.set(
                components::animator(),
                Animator::new(self.animations.clone()),
            );
        }

        entity
            .set(position(), self.pos)
            .set(rotation(), self.rot)
            .set(scale(), self.scale)
            .set(position_offset(), self.pos)
            .set(rotation_offset(), self.rot)
            .set(visible(), Visible::Visible);

        // entity.add_bundle((
        //     self.pos,
        //     self.rot,
        //     self.scale,
        //     PositionOffset(*self.pos),
        //     RotationOffset(*self.rot),
        //     Visible::default(),
        // ));

        entity
    }

    /// Recursively build the whole tree with node as root.
    ///
    /// Requires access to the orignal document.
    ///
    /// Behavior is undefined if a different document is used
    pub fn mount_tree<'a>(
        &self,
        builder: &'a mut EntityBuilder,
        info: &NodeBuildInfo,
    ) -> &'a mut EntityBuilder {
        self.mount(builder, info);

        for child in self.children() {
            let mut subtree = EntityBuilder::new();
            let child = self.document.node(*child);
            child.mount_tree(&mut subtree, info);
            builder.attach(child_of, subtree);
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
