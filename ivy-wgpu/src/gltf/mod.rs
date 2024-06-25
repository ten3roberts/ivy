pub mod node;

use image::{DynamicImage, ImageBuffer, RgbImage, RgbaImage};
use itertools::Itertools;
use ivy_assets::{Asset, AssetCache, AssetDesc};
use ivy_core::profiling::{profile_function, profile_scope};
use ivy_gltf::{DocumentData, GltfPrimitive};
use wgpu::Texture;

use crate::{material::Material, mesh_desc::MeshData, Gpu};

/// Contains the gltf data
pub struct Document {
    pub(crate) mesh_primitives: Vec<Vec<Asset<MeshData>>>,
    pub(crate) materials: Vec<Asset<Material>>,
    pub(crate) images: Vec<Asset<DynamicImage>>,
}

impl Document {
    fn new(gpu: &Gpu, assets: &AssetCache, data: &DocumentData) -> anyhow::Result<Self> {
        profile_function!();

        let textures: Vec<_> = data
            .document
            .images()
            .map(|v| {
                profile_scope!("load_texture");
                let image = gltf::image::Data::from_source(v.source(), None, data.buffer_data())?;

                let image: DynamicImage = match image.format {
                    gltf::image::Format::R8 => todo!(),
                    gltf::image::Format::R8G8 => todo!(),
                    gltf::image::Format::R8G8B8 => {
                        RgbImage::from_raw(image.width, image.height, image.pixels)
                            .unwrap()
                            .into()
                    }
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

                        ImageBuffer::<image::Rgb<u16>, _>::from_raw(
                            image.width,
                            image.height,
                            pixels,
                        )
                        .unwrap()
                        .into()
                    }
                    gltf::image::Format::R16G16B16A16 => todo!(),
                    gltf::image::Format::R32G32B32FLOAT => todo!(),
                    gltf::image::Format::R32G32B32A32FLOAT => todo!(),
                };

                anyhow::Ok(assets.insert(image))
            })
            .try_collect()?;

        let materials: Vec<_> = data
            .document
            .materials()
            .map(|v| {
                profile_scope!("load_material");
                anyhow::Ok(assets.insert(Material::from_gltf(gpu, assets, v, &textures)?))
            })
            .try_collect()?;

        let mesh_primitives: Vec<_> = data
            .document
            .meshes()
            .map(|mesh| -> anyhow::Result<Vec<_>> {
                profile_scope!("load_mesh_primitive");
                mesh.primitives()
                    .map(|primitive| {
                        Ok(assets.insert(MeshData::from_gltf(
                            assets,
                            &primitive,
                            data.buffer_data(),
                        )?))
                    })
                    .try_collect()
            })
            .try_collect()?;

        Ok(Self {
            images: textures,
            mesh_primitives,
            materials,
        })
    }

    pub fn materials(&self) -> &[Asset<Material>] {
        &self.materials
    }

    pub fn mesh_primitives(&self) -> &[Vec<Asset<MeshData>>] {
        &self.mesh_primitives
    }
}

impl AssetDesc<Document> for Asset<ivy_gltf::Document> {
    type Error = anyhow::Error;

    fn load(&self, assets: &AssetCache) -> Result<Asset<Document>, Self::Error> {
        Ok(assets.insert(Document::new(&assets.service(), assets, self.data())?))
    }
}

impl AssetDesc<Document> for Asset<ivy_gltf::DocumentData> {
    type Error = anyhow::Error;

    fn load(&self, assets: &AssetCache) -> Result<Asset<Document>, Self::Error> {
        Ok(assets.insert(Document::new(&assets.service(), assets, self)?))
    }
}

impl AssetDesc<MeshData> for GltfPrimitive {
    type Error = anyhow::Error;

    fn load(&self, assets: &AssetCache) -> Result<Asset<MeshData>, Self::Error> {
        let document: Asset<Document> = assets.try_load(self.data())?;

        document
            .mesh_primitives
            .get(self.mesh_index())
            .ok_or_else(|| anyhow::anyhow!("mesh out of bounds: {}", self.mesh_index(),))?
            .get(self.index())
            .ok_or_else(|| anyhow::anyhow!("mesh primitive out of bounds: {}", self.index(),))
            .cloned()
    }
}
