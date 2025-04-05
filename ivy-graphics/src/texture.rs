use std::{future::Future, ops::Deref, pin::Pin};

use either::Either;
use image::{DynamicImage, ImageBuffer};
use ivy_assets::{
    fs::AssetPath, loadable::ResourceDesc, Asset, AssetCache, AssetDesc, AsyncAssetExt,
};
use ivy_core::palette::Srgba;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ProcessedTextureDesc {
    texture: Box<TextureDesc>,
    processor: StaticTextureProcessor,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProcessedTexture {
    texture: Box<TextureData>,
    processor: StaticTextureProcessor,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum StaticTextureProcessor {
    MetallicRoughness(MetallicRoughnessProcessor),
}

impl TextureProcessor for StaticTextureProcessor {
    fn process(&self, image: DynamicImage) -> DynamicImage {
        match self {
            StaticTextureProcessor::MetallicRoughness(v) => v.process(image),
        }
    }
}

impl From<MetallicRoughnessProcessor> for StaticTextureProcessor {
    fn from(v: MetallicRoughnessProcessor) -> Self {
        Self::MetallicRoughness(v)
    }
}

pub trait TextureProcessor {
    fn process(&self, image: DynamicImage) -> DynamicImage;
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ColorChannel {
    Red,
    Green,
    Blue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct MetallicRoughnessProcessor {
    metallic_channel: Either<ColorChannel, u8>,
    roughness_channel: Either<ColorChannel, u8>,
}

impl MetallicRoughnessProcessor {
    pub fn new(
        metallic_channel: Either<ColorChannel, u8>,
        roughness_channel: Either<ColorChannel, u8>,
    ) -> Self {
        Self {
            metallic_channel,
            roughness_channel,
        }
    }
}

impl TextureProcessor for MetallicRoughnessProcessor {
    fn process(&self, image: DynamicImage) -> DynamicImage {
        let mut image = image.to_rgba8();

        for pixel in image.pixels_mut() {
            let roughness = match self.roughness_channel {
                Either::Left(ColorChannel::Red) => pixel[0],
                Either::Left(ColorChannel::Green) => pixel[0],
                Either::Left(ColorChannel::Blue) => pixel[0],
                Either::Right(v) => v,
            };

            let metallic = match self.metallic_channel {
                Either::Left(ColorChannel::Red) => pixel[0],
                Either::Left(ColorChannel::Green) => pixel[0],
                Either::Left(ColorChannel::Blue) => pixel[0],
                Either::Right(v) => v,
            };

            *pixel = image::Rgba([0, roughness, metallic, 255]);
        }

        image.into()
    }
}

/// Describes a loadable texture
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum TextureDesc {
    Path(AssetPath<DynamicImage>),
    Color(u8, u8, u8, u8),
    Processed(ProcessedTextureDesc),
}

impl TextureDesc {
    pub fn srgba(color: Srgba) -> Self {
        let color = Srgba::<u8>::from_format(color);
        Self::Color(color.red, color.green, color.blue, color.alpha)
    }

    pub fn white() -> Self {
        Self::Color(255, 255, 255, 255)
    }

    pub fn default_normal() -> Self {
        Self::Color(127, 127, 255, 255)
    }

    pub fn process(self, processor: impl Into<StaticTextureProcessor>) -> Self {
        Self::Processed(ProcessedTextureDesc {
            texture: Box::new(self),
            processor: processor.into(),
        })
    }

    // Ah, the beauty of rust at times. It can not figure out the send bound if I use the
    // `async-fn` sugar
    #[allow(clippy::manual_async_fn)]
    fn load_image<'a>(
        &'a self,
        assets: &'a AssetCache,
    ) -> impl Future<Output = anyhow::Result<DynamicImage>> + Send + 'a {
        async move {
            match self {
                Self::Path(v) => Ok(v.load_async(assets).await?.deref().clone()),
                &Self::Color(r, g, b, a) => {
                    Ok(ImageBuffer::from_pixel(1, 1, image::Rgba([r, g, b, a])).into())
                }
                Self::Processed(v) => {
                    // NOTE: load_image, and not load here
                    let original = (Box::pin(async { v.texture.load_image(assets).await })
                        as Pin<Box<dyn Future<Output = anyhow::Result<DynamicImage>> + Send>>)
                        .await?;
                    let processed = v.processor.process(original);
                    Ok(processed)
                }
            }
        }
    }
}

impl ResourceDesc for TextureDesc {
    type Output = TextureData;

    type Error = anyhow::Error;

    async fn load(self, assets: &ivy_assets::AssetCache) -> Result<Self::Output, Self::Error> {
        let texture = match self {
            TextureDesc::Path(path) => TextureData::Content(path.load_async(assets).await?),
            TextureDesc::Color(r, g, b, a) => TextureData::Color(image::Rgba([r, g, b, a])),
            TextureDesc::Processed(v) => {
                // NOTE: ensure we don't recurse with assets here, and use raw uncached images on
                // the way down (and only loading the base image into the asset cache)
                let image = v.texture.load_image(assets).await?;
                TextureData::Content(assets.insert(v.processor.process(image)))
            }
        };

        Ok(texture)
    }
}

/// Describes a texture
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TextureData {
    Content(Asset<DynamicImage>),
    Color(image::Rgba<u8>),
    Processed(ProcessedTexture),
}

impl TextureData {
    pub fn content(content: Asset<DynamicImage>) -> Self {
        Self::Content(content)
    }

    pub fn process(self, processor: impl Into<StaticTextureProcessor>) -> Self {
        Self::Processed(ProcessedTexture {
            texture: Box::new(self),
            processor: processor.into(),
        })
    }

    pub fn srgba(color: Srgba) -> Self {
        let color = Srgba::<u8>::from_format(color);
        Self::Color(image::Rgba([
            color.red,
            color.green,
            color.blue,
            color.alpha,
        ]))
    }

    pub fn white() -> Self {
        Self::Color(image::Rgba([255, 255, 255, 255]))
    }

    pub fn default_normal() -> Self {
        Self::Color(image::Rgba([127, 127, 255, 255]))
    }

    pub fn label(&self) -> String {
        match self {
            TextureData::Content(_) => "content".to_string(),
            TextureData::Color(_) => "color".to_string(),
            TextureData::Processed(v) => v.texture.label().to_string(),
        }
    }

    fn load_image(&self) -> anyhow::Result<DynamicImage> {
        match self {
            TextureData::Content(v) => Ok((**v).clone()),
            TextureData::Color(v) => Ok(ImageBuffer::from_pixel(1, 1, *v).into()),
            TextureData::Processed(v) => {
                let original = v.texture.load_image()?;
                let processed = v.processor.process(original);
                Ok(processed)
            }
        }
    }
}

impl AssetDesc for TextureData {
    type Output = DynamicImage;
    type Error = anyhow::Error;

    fn create(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<DynamicImage>, Self::Error> {
        Ok(assets.insert(self.load_image()?))
    }
}
