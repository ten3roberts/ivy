pub mod animation;
pub mod components;

use animation::skin::Skin;
use animation::Animation;
use futures::StreamExt;
use futures::TryStreamExt;
use glam::Mat4;
use glam::Quat;
use image::{DynamicImage, ImageBuffer, RgbImage, RgbaImage};
use itertools::Itertools;
use ivy_assets::fs::AsyncAssetFromPath;
use ivy_assets::AssetId;
use ivy_core::TransformBundle;
use ivy_profiling::profile_scope;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::{collections::HashMap, path::Path};
use tracing::Instrument;

use gltf::Gltf;
use ivy_assets::{Asset, AssetCache};

/// An in memory representation of a gltf document and binary buffer data
pub struct DocumentData {
    gltf: Gltf,

    named_meshes: HashMap<String, usize>,
    named_materials: HashMap<String, usize>,
    named_nodes: HashMap<String, usize>,
    named_animations: HashMap<String, usize>,

    buffer_data: Arc<Vec<gltf::buffer::Data>>,
    images: Vec<Asset<DynamicImage>>,

    skins: Vec<Asset<Skin>>,
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

    pub fn images(&self) -> &[Asset<DynamicImage>] {
        &self.images
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
    async fn load(assets: &AssetCache, path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let bytes: Asset<Vec<u8>> = assets.try_load_async(path.as_ref()).await?;

        let mut gltf = Gltf::from_slice(&bytes)?;

        let buffer_data: Vec<_> = gltf
            .document
            .buffers()
            .map(|v| {
                tracing::info!("loading buffer data");
                // TODO: load using assets
                gltf::buffer::Data::from_source_and_blob(v.source(), None, &mut gltf.blob)
            })
            .try_collect()?;

        let buffer_data = Arc::new(buffer_data);

        let images: Vec<_> = futures::stream::iter(gltf.images().enumerate())
            .map(|(i, v)| {
                let image = gltf::image::Data::from_source(v.source(), None, &buffer_data);
                async {
                    let image = image?;
                    let image = async_std::task::spawn_blocking(|| load_image(image)).await?;
                    anyhow::Ok(assets.insert(image))
                }
                .instrument(tracing::info_span!("load_image", i))
            })
            .boxed()
            .buffered(4)
            .try_collect()
            .await?;

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

        let named_animations = gltf
            .document
            .animations()
            .enumerate()
            .filter_map(|(i, v)| Some((v.name().map(ToString::to_string)?, i)))
            .collect();

        let skins = Skin::load_from_document(assets, &gltf.document, &buffer_data)?;

        let data = assets.insert(DocumentData {
            gltf,
            named_meshes,
            named_materials,
            named_nodes,
            buffer_data,
            images,
            named_animations,
            skins,
        });

        // let skins: Vec<_> = data
        //     .gltf
        //     .document
        //     .skins()
        //     .map(|v| anyhow::Ok(assets.insert(Skin::from_gltf(data.clone(), v)?)))
        //     .try_collect()?;

        // let animations = data
        //     .gltf
        //     .document
        //     .animations()
        //     .flat_map(|v| Animation::from_gltf(v, &skins, &data.buffer_data))
        //     .map(|(k, v)| (k, assets.insert(v)))
        //     .fold(BTreeMap::new(), |mut acc, (skin, v)| {
        //         acc.entry(skin).or_insert_with(Vec::new).push(v);
        //         acc
        //     });

        Ok(Self { data })
    }

    pub fn meshes(&self) -> impl Iterator<Item = GltfMesh> + '_ {
        self.data
            .meshes()
            .map(|v| GltfMesh::new(self.data.clone(), v))
    }

    pub fn materials(&self) -> impl Iterator<Item = GltfMaterial> + '_ {
        self.data
            .materials()
            .map(|v| GltfMaterial::new(self.data.clone(), v))
    }

    pub fn nodes(&self) -> impl Iterator<Item = GltfNode> + '_ {
        self.data
            .nodes()
            .map(|v| GltfNode::new(self.data.clone(), v))
    }

    pub fn data(&self) -> &DocumentData {
        &self.data
    }

    pub fn mesh(&self, index: usize) -> Option<GltfMesh> {
        self.data
            .meshes()
            .nth(index)
            .map(|v| GltfMesh::new(self.data.clone(), v))
    }

    pub fn material(&self, index: usize) -> Option<GltfMaterial> {
        self.data
            .materials()
            .nth(index)
            .map(|v| GltfMaterial::new(self.data.clone(), v))
    }

    pub fn node(&self, index: usize) -> Option<GltfNode> {
        self.data
            .nodes()
            .nth(index)
            .map(|v| GltfNode::new(self.data.clone(), v))
    }

    pub fn find_mesh(&self, name: impl AsRef<str>) -> Option<GltfMesh> {
        tracing::info!(?self.data.named_meshes);
        self.data
            .named_meshes
            .get(name.as_ref())
            .map(|&index| self.mesh(index).unwrap())
    }

    pub fn find_material(&self, name: impl AsRef<str>) -> Option<GltfMaterial> {
        self.data
            .named_materials
            .get(name.as_ref())
            .map(|&index| self.material(index).unwrap())
    }

    pub fn find_node(&self, name: impl AsRef<str>) -> Option<GltfNode> {
        self.data
            .named_nodes
            .get(name.as_ref())
            .map(|&index| self.node(index).unwrap())
    }
}

fn load_image(image: gltf::image::Data) -> Result<DynamicImage, anyhow::Error> {
    profile_scope!("load_texture");

    let image: DynamicImage = match image.format {
        gltf::image::Format::R8 => todo!(),
        gltf::image::Format::R8G8 => todo!(),
        gltf::image::Format::R8G8B8 => RgbImage::from_raw(image.width, image.height, image.pixels)
            .unwrap()
            .into(),
        gltf::image::Format::R8G8B8A8 => {
            RgbaImage::from_raw(image.width, image.height, image.pixels)
                .unwrap()
                .into()
        }
        gltf::image::Format::R16 => todo!(),
        gltf::image::Format::R16G16 => todo!(),
        gltf::image::Format::R16G16B16 => {
            let pixels = image
                .pixels
                .chunks_exact(6)
                .flat_map(|v| {
                    let r = u16::from_le_bytes([v[0], v[1]]);
                    let g = u16::from_le_bytes([v[2], v[3]]);
                    let b = u16::from_le_bytes([v[4], v[5]]);

                    [r, g, b]
                })
                .collect::<Vec<_>>();

            ImageBuffer::<image::Rgb<u16>, _>::from_raw(image.width, image.height, pixels)
                .unwrap()
                .into()
        }
        gltf::image::Format::R16G16B16A16 => todo!(),
        gltf::image::Format::R32G32B32FLOAT => todo!(),
        gltf::image::Format::R32G32B32A32FLOAT => todo!(),
    };

    Ok(image)
}

impl AsyncAssetFromPath for Document {
    type Error = anyhow::Error;

    async fn load_from_path(path: &Path, assets: &AssetCache) -> Result<Asset<Self>, Self::Error> {
        Document::load(assets, path).await.map(|v| assets.insert(v))
    }
}

/// References a mesh primitive in a gltf document
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GltfPrimitive {
    data: Asset<DocumentData>,
    mesh_index: usize,
    index: usize,
}

impl GltfPrimitive {
    pub fn new(data: Asset<DocumentData>, mesh: &GltfMesh, value: gltf::Primitive) -> Self {
        Self {
            data,
            mesh_index: mesh.index(),
            index: value.index(),
        }
    }

    pub fn material(&self) -> GltfMaterial {
        GltfMaterial::new(
            self.data.clone(),
            self.data
                .primitive((self.mesh_index, self.index))
                .unwrap()
                .material(),
        )
    }
}

/// References a mesh in a gltf document
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GltfMesh {
    data: Asset<DocumentData>,
    index: usize,
}

impl GltfMesh {
    pub fn new(data: Asset<DocumentData>, value: gltf::Mesh) -> Self {
        Self {
            data,
            index: value.index(),
        }
    }

    pub fn name(&self) -> Option<&str> {
        self.data.mesh(self.index).and_then(|v| v.name())
    }

    pub fn primitives(&self) -> impl Iterator<Item = GltfPrimitive> + '_ {
        self.data
            .mesh(self.index)
            .unwrap()
            .primitives()
            .map(|v| GltfPrimitive::new(self.data.clone(), self, v))
    }
}

/// References a material in a gltf document
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GltfMaterial {
    data: Asset<DocumentData>,
    index: usize,
}

impl GltfMaterial {
    pub fn new(data: Asset<DocumentData>, value: gltf::Material) -> Self {
        Self {
            data,
            index: value.index().unwrap(),
        }
    }

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
    pub fn new(data: Asset<DocumentData>, value: gltf::Node) -> Self {
        Self {
            data,
            index: value.index(),
        }
    }

    pub fn name(&self) -> Option<&str> {
        self.data.node(self.index).and_then(|v| v.name())
    }

    pub fn mesh(&self) -> Option<GltfMesh> {
        let node = self.data.node(self.index).unwrap();
        Some(GltfMesh::new(self.data.clone(), node.mesh()?))
    }

    pub fn transform(&self) -> TransformBundle {
        let (pos, rot, scale) = self.data.node(self.index).unwrap().transform().decomposed();
        TransformBundle::new(pos.into(), Quat::from_array(rot), scale.into())
    }

    pub fn transform_matrix(&self) -> Mat4 {
        let matrix = self.data.node(self.index).unwrap().transform().matrix();

        Mat4::from_cols_array_2d(&matrix)
    }

    pub fn children(&self) -> impl Iterator<Item = GltfNode> + '_ {
        self.data
            .node(self.index)
            .unwrap()
            .children()
            .map(move |v| GltfNode::new(self.data.clone(), v))
    }

    pub fn skin(&self) -> Option<Asset<Skin>> {
        let skin = self.data.node(self.index).unwrap().skin()?;

        Some(self.data.skins[skin.index()].clone())
    }
}

macro_rules! gltf_node_impl {
    ($ty: ty, $name: ident) => {
        impl $ty {
            #[inline]
            pub fn index(&self) -> usize {
                self.index
            }

            pub fn data(&self) -> &Asset<DocumentData> {
                &self.data
            }
        }
    };
}

gltf_node_impl! { GltfMesh, mesh }
gltf_node_impl! { GltfNode, node }
gltf_node_impl! { GltfMaterial, material }

impl GltfPrimitive {
    #[inline]
    pub fn mesh_index(&self) -> usize {
        self.mesh_index
    }

    /// **Note**: Refers to the index inside the mesh, not globally
    pub fn index(&self) -> usize {
        self.index
    }

    pub fn data(&self) -> &Asset<DocumentData> {
        &self.data
    }
}
