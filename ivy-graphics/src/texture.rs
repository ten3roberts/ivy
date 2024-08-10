use image::DynamicImage;
use ivy_assets::Asset;

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
}
