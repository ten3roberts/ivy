use image::DynamicImage;
use ivy_assets::{Asset, AssetCache, AssetDesc};
use ivy_core::profiling::profile_function;
use ivy_wgpu_types::texture::{texture_from_image, TextureFromColor, TextureFromImageDesc};
use wgpu::{Texture, TextureFormat};

/// Describes a texture
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TextureDesc {
    Path(String),
    Content(Asset<DynamicImage>),
    Color(image::Rgba<u8>),
}

impl From<String> for TextureDesc {
    fn from(v: String) -> Self {
        Self::Path(v)
    }
}

impl From<&str> for TextureDesc {
    fn from(v: &str) -> Self {
        Self::Path(v.into())
    }
}

impl TextureDesc {
    pub fn path(path: impl Into<String>) -> Self {
        Self::Path(path.into())
    }

    pub fn content(content: Asset<DynamicImage>) -> Self {
        Self::Content(content)
    }

    pub fn white() -> Self {
        Self::Color(image::Rgba([255, 255, 255, 255]))
    }

    pub fn default_normal() -> Self {
        Self::Color(image::Rgba([127, 127, 255, 255]))
    }

    pub fn load(
        &self,
        assets: &AssetCache,
        format: TextureFormat,
    ) -> Result<Asset<Texture>, image::ImageError> {
        profile_function!("TextureDesc::load");
        let gpu = assets.service();

        match self {
            TextureDesc::Path(v) => {
                let image = image::open(v)?;
                Ok(assets.insert(
                    texture_from_image(
                        &gpu,
                        assets,
                        &image,
                        TextureFromImageDesc {
                            label: v.clone().into(),
                            format,
                            ..Default::default()
                        },
                    )
                    .unwrap(),
                ))
            }
            TextureDesc::Content(image) => Ok(assets.insert(
                texture_from_image(
                    &gpu,
                    assets,
                    image,
                    TextureFromImageDesc {
                        label: "content".into(),
                        format,
                        ..Default::default()
                    },
                )
                .unwrap(),
            )),
            TextureDesc::Color(v) => Ok(assets.load(&TextureFromColor { color: v.0, format })),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct TextureAndKindDesc {
    texture: TextureDesc,
    format: TextureFormat,
}

impl TextureAndKindDesc {
    pub(crate) fn new(texture: TextureDesc, format: TextureFormat) -> Self {
        Self { texture, format }
    }
}

impl AssetDesc<Texture> for TextureAndKindDesc {
    type Error = image::ImageError;

    fn load(&self, assets: &AssetCache) -> Result<Asset<Texture>, Self::Error> {
        self.texture.load(assets, self.format)
    }
}
