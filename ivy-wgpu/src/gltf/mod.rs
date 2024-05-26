use std::{ops::Deref, path::Path};

use gltf::Gltf;
use image::{DynamicImage, Rgba32FImage, RgbaImage};
use itertools::Itertools;
use ivy_assets::{Asset, AssetCache, AssetKey};
use ivy_gltf::{DocumentData, GltfMaterial, GltfMesh};

use crate::{
    graphics::{
        material::Material,
        texture::{Texture, TextureFromPath},
        Mesh,
    },
    material::MaterialData,
    Gpu,
};

/// Contains the gltf data
pub struct Document {
    meshes: Vec<Asset<Mesh>>,
    materials: Vec<Asset<Material>>,
    images: Vec<Asset<Texture>>,
}

impl Document {
    fn new(gpu: &Gpu, assets: &AssetCache, data: &DocumentData) -> anyhow::Result<Self> {
        let textures: Vec<_> = data
            .document
            .images()
            .map(|v| {
                tracing::info!(?v, "import image");
                let image = gltf::image::Data::from_source(v.source(), None, data.buffer_data())?;

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
            .map(|mesh| {
                assets.insert(Mesh::from_gltf(
                    gpu,
                    assets,
                    mesh,
                    data.buffer_data(),
                    &materials,
                ))
            })
            .collect_vec();

        Ok(Self {
            images: textures,
            meshes,
            materials,
        })
    }
}

impl AssetKey<Document> for Asset<ivy_gltf::Document> {
    type Error = anyhow::Error;

    fn load(&self, assets: &AssetCache) -> Result<Asset<Document>, Self::Error> {
        Ok(assets.insert(Document::new(&assets.service(), assets, self.data())?))
    }
}

impl AssetKey<Document> for Asset<ivy_gltf::DocumentData> {
    type Error = anyhow::Error;

    fn load(&self, assets: &AssetCache) -> Result<Asset<Document>, Self::Error> {
        Ok(assets.insert(Document::new(&assets.service(), assets, &self)?))
    }
}

impl AssetKey<Mesh> for GltfMesh {
    type Error = anyhow::Error;

    fn load(&self, assets: &AssetCache) -> Result<Asset<Mesh>, Self::Error> {
        let document: Asset<Document> = assets.try_load(self.data())?;

        document.meshes.get(self.index()).cloned().ok_or_else(|| {
            anyhow::anyhow!(
                "mesh index out of bounds: {} >= {}",
                self.index(),
                document.meshes.len()
            )
        })
    }
}

impl AssetKey<Material> for GltfMaterial {
    type Error = anyhow::Error;

    fn load(&self, assets: &AssetCache) -> Result<Asset<Material>, Self::Error> {
        let document: Asset<Document> = assets.try_load(self.data())?;

        document
            .materials
            .get(self.index())
            .cloned()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "material index out of bounds: {} >= {}",
                    self.index(),
                    document.materials.len()
                )
            })
    }
}
