pub mod animation;
pub mod components;

use std::{borrow::Cow, collections::HashMap, fs, future::Future, io, path::Path, sync::Arc};

use animation::skin::Skin;
use anyhow::Context;
use futures::{stream, StreamExt, TryStreamExt};
use glam::{Mat4, Quat, U16Vec4, Vec2, Vec3, Vec4};
use gltf::{buffer, Gltf};
use image::{DynamicImage, ImageFormat};
use itertools::Itertools;
use ivy_assets::{
    fs::AssetPath, loadable::ResourceFromPath, Asset, AssetCache, AssetDesc, AsyncAssetExt,
};
use ivy_core::components::TransformBundle;
use ivy_graphics::mesh::{MeshData, TANGENT_ATTRIBUTE};
use ivy_profiling::{profile_function, profile_scope};
use rayon::iter::{ParallelBridge, ParallelIterator};

/// An in memory representation of a gltf document and binary buffer data
pub struct DocumentData {
    gltf: Gltf,

    named_meshes: HashMap<String, usize>,
    named_materials: HashMap<String, usize>,
    named_nodes: HashMap<String, usize>,

    buffer_data: Arc<Vec<gltf::buffer::Data>>,
    images: Vec<Asset<DynamicImage>>,
    mesh_data: Vec<Vec<Asset<MeshData>>>,

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

    pub fn mesh_data(&self) -> &[Vec<Asset<MeshData>>] {
        &self.mesh_data
    }

    pub fn gltf(&self) -> &Gltf {
        &self.gltf
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
        let path = path.as_ref();
        let bytes: Asset<Vec<u8>> = AssetPath::new(path).load_async(assets).await?;

        let mut gltf = Gltf::from_slice(&bytes)?;

        let buffer_data: Vec<_> = gltf
            .document
            .buffers()
            .map(|v| {
                profile_scope!("load_buffer_data");
                // TODO: load using assets
                gltf::buffer::Data::from_source_and_blob(v.source(), None, &mut gltf.blob)
            })
            .try_collect()?;

        let buffer_data = Arc::new(buffer_data);

        let images = gltf.images().collect_vec();
        let mut images: Vec<_> = stream::iter(images.iter().enumerate())
            .map(|(i, v)| {
                let buffer_data = buffer_data.clone();
                async move {
                    // let image = gltf::image::Data::from_source(v.source(), None, &buffer_data);
                    let image = load_image_data(v.source(), None, &buffer_data)
                        .await
                        .with_context(|| format!("Failed to load image {:?}", v.name()))?;

                    anyhow::Ok((i, assets.insert(image)))
                }
            })
            .boxed()
            .buffered(8)
            .try_collect()
            .await?;

        images.sort_by_key(|v| v.0);
        let images = images.into_iter().map(|v| v.1).collect_vec();

        let meshes: Vec<_> = futures::stream::iter(gltf.meshes())
            .map(|v| {
                let buffer_data = buffer_data.clone();
                async move {
                    let primitives = futures::stream::iter(v.primitives())
                        .then(|primitive| {
                            let buffer_data = buffer_data.clone();
                            async move {
                                anyhow::Ok(assets.insert(
                                    mesh_from_gltf(assets, &primitive, &buffer_data).await?,
                                ))
                            }
                        })
                        .try_collect()
                        .await?;

                    anyhow::Ok(primitives)
                }
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

        let skins = Skin::load_from_document(assets, &gltf.document, &buffer_data)?;

        let data = assets.insert(DocumentData {
            gltf,
            named_meshes,
            named_materials,
            named_nodes,
            buffer_data,
            images,
            skins,
            mesh_data: meshes,
        });

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

/// NOTE: this is a copy of [`gltf::image::Data::from_source`] that returns the `DynamicImage`
/// directly, saving costly back and forth conversion
/// Construct an image data object by reading the given source.
/// If `base` is provided, then external filesystem references will
/// be resolved from this directory.
async fn load_image_data(
    source: gltf::image::Source<'_>,
    base: Option<&Path>,
    buffer_data: &[buffer::Data],
) -> gltf::Result<DynamicImage> {
    let guess_format = |encoded_image: &[u8]| match image::guess_format(encoded_image) {
        Ok(ImageFormat::Png) => Some(ImageFormat::Png),
        Ok(ImageFormat::Jpeg) => Some(ImageFormat::Jpeg),
        _ => None,
    };

    let decoded_image = match source {
        gltf::image::Source::Uri { uri, mime_type } if base.is_some() => match Scheme::parse(uri) {
            Scheme::Data(Some(annoying_case), base64) => {
                let encoded_image = base64::decode(base64).map_err(gltf::Error::Base64)?;
                let encoded_format = match annoying_case {
                    "image/png" => ImageFormat::Png,
                    "image/jpeg" => ImageFormat::Jpeg,
                    _ => match guess_format(&encoded_image) {
                        Some(format) => format,
                        None => return Err(gltf::Error::UnsupportedImageEncoding),
                    },
                };

                image::load_from_memory_with_format(&encoded_image, encoded_format)?
            }
            Scheme::Unsupported => return Err(gltf::Error::UnsupportedScheme),
            _ => {
                let encoded_image = Scheme::read(base, uri)?;
                let encoded_format = match mime_type {
                    Some("image/png") => ImageFormat::Png,
                    Some("image/jpeg") => ImageFormat::Jpeg,
                    Some(_) => match guess_format(&encoded_image) {
                        Some(format) => format,
                        None => return Err(gltf::Error::UnsupportedImageEncoding),
                    },
                    None => match uri.rsplit('.').next() {
                        Some("png") => ImageFormat::Png,
                        Some("jpg") | Some("jpeg") => ImageFormat::Jpeg,
                        _ => match guess_format(&encoded_image) {
                            Some(format) => format,
                            None => return Err(gltf::Error::UnsupportedImageEncoding),
                        },
                    },
                };

                async_std::task::spawn_blocking(move || {
                    image::load_from_memory_with_format(&encoded_image, encoded_format)
                })
                .await?
            }
        },
        gltf::image::Source::View { view, mime_type } => {
            let parent_buffer_data = &buffer_data[view.buffer().index()].0;
            let begin = view.offset();
            let end = begin + view.length();
            let encoded_image = parent_buffer_data[begin..end].to_vec();
            let encoded_format = match mime_type {
                "image/png" => ImageFormat::Png,
                "image/jpeg" => ImageFormat::Jpeg,
                _ => match guess_format(&encoded_image) {
                    Some(format) => format,
                    None => return Err(gltf::Error::UnsupportedImageEncoding),
                },
            };
            async_std::task::spawn_blocking(move || {
                profile_scope!("decode image");
                image::load_from_memory_with_format(&encoded_image, encoded_format)
            })
            .await?
        }
        _ => return Err(gltf::Error::ExternalReferenceInSliceImport),
    };

    Ok(decoded_image)
}

impl ResourceFromPath for Document {
    type Error = anyhow::Error;

    async fn load(path: AssetPath<Self>, assets: &AssetCache) -> Result<Self, Self::Error> {
        Document::load(assets, path.path()).await
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

    pub fn material(&self) -> gltf::Material {
        self.data.material(self.index).unwrap()
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

impl AssetDesc for GltfPrimitive {
    type Output = MeshData;
    type Error = anyhow::Error;

    fn create(&self, _: &AssetCache) -> Result<Asset<MeshData>, Self::Error> {
        self.data()
            .mesh_data()
            .get(self.mesh_index())
            .ok_or_else(|| anyhow::anyhow!("mesh out of bounds: {}", self.mesh_index(),))?
            .get(self.index())
            .ok_or_else(|| anyhow::anyhow!("mesh primitive out of bounds: {}", self.index(),))
            .cloned()
    }
}

pub(crate) fn mesh_from_gltf(
    _: &AssetCache,
    primitive: &gltf::Primitive,
    buffer_data: &[gltf::buffer::Data],
) -> impl Future<Output = anyhow::Result<MeshData>> {
    profile_function!();

    let reader = primitive.reader(|buffer| Some(&buffer_data[buffer.index()]));

    let indices = reader
        .read_indices()
        .into_iter()
        .flat_map(|val| val.into_u32())
        .collect_vec();

    let pos = reader
        .read_positions()
        .into_iter()
        .flatten()
        .map(Vec3::from);

    let normals = reader.read_normals().into_iter().flatten().map(Vec3::from);

    let tangents = reader.read_tangents().map(|v| v.map(Vec4::from));

    let joints = reader
        .read_joints(0)
        .into_iter()
        .flat_map(|v| v.into_u16())
        .map(U16Vec4::from)
        .collect_vec();

    let weights = reader
        .read_weights(0)
        .into_iter()
        .flat_map(|v| v.into_f32())
        .map(Vec4::from)
        .collect_vec();

    let texcoord = reader
        .read_tex_coords(0)
        .into_iter()
        .flat_map(|val| val.into_f32())
        .map(Vec2::from);

    let this = MeshData::skinned(indices, pos, texcoord, normals, joints, weights);
    let this = if let Some(tangents) = tangents {
        tracing::info!("using mesh tangents");
        this.with_attribute(TANGENT_ATTRIBUTE, tangents)
    } else {
        this
    };

    async move {
        let this = async_std::task::spawn_blocking(move || this.with_generated_tangents()).await?;

        Ok(this)
    }
}

/// Represents the set of URI schemes the importer supports.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
enum Scheme<'a> {
    /// `data:[<media type>];base64,<data>`.
    Data(Option<&'a str>, &'a str),

    /// `file:[//]<absolute file path>`.
    ///
    /// Note: The file scheme does not implement authority.
    File(&'a str),

    /// `../foo`, etc.
    Relative(Cow<'a, str>),

    /// Placeholder for an unsupported URI scheme identifier.
    Unsupported,
}

impl Scheme<'_> {
    fn parse(uri: &str) -> Scheme<'_> {
        if uri.contains(':') {
            if let Some(rest) = uri.strip_prefix("data:") {
                let mut it = rest.split(";base64,");

                match (it.next(), it.next()) {
                    (match0_opt, Some(match1)) => Scheme::Data(match0_opt, match1),
                    (Some(match0), _) => Scheme::Data(None, match0),
                    _ => Scheme::Unsupported,
                }
            } else if let Some(rest) = uri.strip_prefix("file://") {
                Scheme::File(rest)
            } else if let Some(rest) = uri.strip_prefix("file:") {
                Scheme::File(rest)
            } else {
                Scheme::Unsupported
            }
        } else {
            Scheme::Relative(urlencoding::decode(uri).unwrap())
        }
    }

    fn read(base: Option<&Path>, uri: &str) -> gltf::Result<Vec<u8>> {
        match Scheme::parse(uri) {
            // The path may be unused in the Scheme::Data case
            // Example: "uri" : "data:application/octet-stream;base64,wsVHPgA...."
            Scheme::Data(_, base64) => base64::decode(base64).map_err(gltf::Error::Base64),
            Scheme::File(path) if base.is_some() => read_to_end(path),
            Scheme::Relative(path) if base.is_some() => read_to_end(base.unwrap().join(&*path)),
            Scheme::Unsupported => Err(gltf::Error::UnsupportedScheme),
            _ => Err(gltf::Error::ExternalReferenceInSliceImport),
        }
    }
}

fn read_to_end<P>(path: P) -> gltf::Result<Vec<u8>>
where
    P: AsRef<Path>,
{
    use io::Read;
    let file = fs::File::open(path.as_ref()).map_err(gltf::Error::Io)?;
    // Allocate one extra byte so the buffer doesn't need to grow before the
    // final `read` call at the end of the file.  Don't worry about `usize`
    // overflow because reading will fail regardless in that case.
    let length = file.metadata().map(|x| x.len() + 1).unwrap_or(0);
    let mut reader = io::BufReader::new(file);
    let mut data = Vec::with_capacity(length as usize);
    reader.read_to_end(&mut data).map_err(gltf::Error::Io)?;
    Ok(data)
}
