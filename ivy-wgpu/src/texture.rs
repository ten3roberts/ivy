use std::convert::Infallible;

use image::{DynamicImage, ImageError};
use ivy_assets::{Asset, AssetKey};

use crate::graphics::texture::Texture;

/// Describes a texture
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TextureDesc {
    Path(String),
    Content(Asset<DynamicImage>),
}

impl TextureDesc {
    pub fn path(path: impl Into<String>) -> Self {
        Self::Path(path.into())
    }

    pub fn content(content: Asset<DynamicImage>) -> Self {
        Self::Content(content)
    }
}

impl AssetKey<Texture> for TextureDesc {
    type Error = ImageError;

    fn load(
        &self,
        assets: &ivy_assets::AssetCache,
    ) -> Result<ivy_assets::Asset<Texture>, Self::Error> {
        let gpu = assets.service();
        let texture = match self {
            TextureDesc::Path(v) => {
                let image = image::open(v)?;
                Texture::from_image(&gpu, &image)
            }
            TextureDesc::Content(v) => Texture::from_image(&gpu, v),
        };

        Ok(assets.insert(texture))
    }
}
