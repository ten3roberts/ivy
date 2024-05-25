use std::path::Path;

use gltf::Gltf;
use image::{DynamicImage, Rgba32FImage, RgbaImage};
use itertools::Itertools;
use ivy_assets::{Asset, AssetCache, AssetKey};

use crate::{
    graphics::{
        material::Material,
        texture::{Texture, TextureFromPath},
        Mesh,
    },
    Gpu,
};

/// An in memory representation of a gltf document
struct DocumentData {
    document: gltf::Document,
    // buffer_data: Vec<gltf::buffer::Data>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GltfImageData {
    data: Asset<DocumentData>,
    index: usize,
}

pub struct GltfMesh {
    data: Asset<DocumentData>,
    index: usize,
}

pub struct GltfMaterial {
    data: Asset<DocumentData>,
    index: usize,
}

pub struct GltfNode {
    data: Asset<DocumentData>,
    index: usize,
}

impl DocumentData {
    pub fn new(assets: &AssetCache, path: impl AsRef<Path>) -> anyhow::Result<Self> {
        let bytes: Asset<Vec<u8>> = assets.load(path.as_ref());

        let gltf = Gltf::from_slice(&bytes)?;

        let buffer_data: Vec<_> = gltf
            .document
            .buffers()
            .map(|v| {
                tracing::info!(?v, "import buffer");
                // TODO: load using assets
                gltf::buffer::Data::from_source(v.source(), None)
            })
            .try_collect()?;

        Ok(Self {
            document: gltf.document,
            // buffer_data,
        })
    }
}

/// Contains the gltf data
pub struct Document {
    meshes: Vec<Asset<Mesh>>,
    materials: Vec<Asset<Material>>,
    images: Vec<Asset<Texture>>,
}

impl Document {
    pub fn new(gpu: &Gpu, assets: &AssetCache, data: &Gltf) -> anyhow::Result<Self> {
        let buffer_data: Vec<_> = data
            .document
            .buffers()
            .map(|v| {
                tracing::info!(?v, "import buffer");
                gltf::buffer::Data::from_source(v.source(), None)
            })
            .try_collect()?;

        let textures: Vec<_> = data
            .document
            .images()
            .map(|v| {
                tracing::info!(?v, "import image");
                let image = gltf::image::Data::from_source(v.source(), None, &buffer_data)?;

                let image: DynamicImage = match image.format {
                    gltf::image::Format::R8 => todo!(),
                    gltf::image::Format::R8G8 => todo!(),
                    gltf::image::Format::R8G8B8 => todo!(),
                    gltf::image::Format::R8G8B8A8 => {
                        RgbaImage::from_raw(image.width, image.height, image.pixels)
                            .unwrap()
                            .into()
                    }
                    gltf::image::Format::R16 => todo!(),
                    gltf::image::Format::R16G16 => todo!(),
                    gltf::image::Format::R16G16B16 => todo!(),
                    gltf::image::Format::R16G16B16A16 => todo!(),
                    gltf::image::Format::R32G32B32FLOAT => todo!(),
                    gltf::image::Format::R32G32B32A32FLOAT => todo!(),
                };

                anyhow::Ok(assets.insert(Texture::from_image(gpu, &image)))
            })
            .try_collect()?;

        let materials: Vec<_> = data
            .document
            .materials()
            .map(|v| anyhow::Ok(assets.insert(Material::from_gltf(gpu, assets, v, &textures)?)))
            .try_collect()?;

        let meshes: Vec<_> = data
            .document
            .meshes()
            .map(|mesh| assets.insert(Mesh::from_gltf(gpu, assets, mesh, &buffer_data, &materials)))
            .collect_vec();

        Ok(Self {
            images: textures,
            meshes,
            materials,
        })
    }
}
