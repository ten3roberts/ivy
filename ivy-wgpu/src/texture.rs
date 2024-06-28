use image::{DynamicImage, GenericImageView};
use ivy_assets::{Asset, AssetCache, AssetDesc};
use ivy_wgpu_types::{
    texture::{max_mip_levels, texture_from_image, TextureFromColor, TextureFromImageDesc},
    Gpu,
};
use wgpu::{Texture, TextureFormat};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureKind {
    Srgba,
    Uniform,
}

/// Describes a texture
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TextureDesc {
    Path(String),
    Content(Asset<DynamicImage>),
    Color(image::Rgba<u8>),
}

impl TextureDesc {
    pub fn path(path: impl Into<String>) -> Self {
        Self::Path(path.into())
    }

    pub fn content(content: Asset<DynamicImage>) -> Self {
        Self::Content(content)
    }

    pub fn default_normal() -> Self {
        Self::Color(image::Rgba([127, 127, 255, 255]))
    }

    pub fn load(
        &self,
        assets: &AssetCache,
        kind: TextureKind,
    ) -> Result<Asset<Texture>, image::ImageError> {
        let format = match kind {
            TextureKind::Srgba => TextureFormat::Rgba8UnormSrgb,
            TextureKind::Uniform => TextureFormat::Rgba8Unorm,
        };

        let gpu = assets.service();

        match self {
            TextureDesc::Path(v) => {
                let image = image::open(v)?;
                Ok(assets.insert(texture_from_image(
                    &gpu,
                    assets,
                    &image,
                    TextureFromImageDesc {
                        label: v.clone().into(),
                        format,
                        ..Default::default()
                    },
                )))
            }
            TextureDesc::Content(image) => Ok(assets.insert(texture_from_image(
                &gpu,
                assets,
                image,
                TextureFromImageDesc {
                    label: "content".into(),
                    format,
                    ..Default::default()
                },
            ))),
            TextureDesc::Color(v) => Ok(assets.load(&TextureFromColor { color: v.0, format })),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct TextureAndKindDesc {
    texture: TextureDesc,
    kind: TextureKind,
}

impl TextureAndKindDesc {
    pub(crate) fn new(texture: TextureDesc, kind: TextureKind) -> Self {
        Self { texture, kind }
    }
}

impl AssetDesc<Texture> for TextureAndKindDesc {
    type Error = image::ImageError;

    fn load(&self, assets: &AssetCache) -> Result<Asset<Texture>, Self::Error> {
        self.texture.load(assets, self.kind)
    }
}
