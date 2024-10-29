use image::{DynamicImage, ImageBuffer};
use ivy_assets::{Asset, AssetDesc, AsyncAssetDesc, DynAsyncAssetDesc, DynAssetDesc};
use ivy_core::palette::Srgba;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ProcessedTexture {
    texture: TextureDesc,
    processor: StaticTextureProcessor,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
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
pub enum ColorChannel {
    Red,
    Green,
    Blue,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct MetallicRoughnessProcessor {
    metallic_channel: Option<ColorChannel>,
    roughness_channel: Option<ColorChannel>,
}

impl MetallicRoughnessProcessor {
    pub fn new(
        metallic_channel: Option<ColorChannel>,
        roughness_channel: Option<ColorChannel>,
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
                Some(ColorChannel::Red) => pixel[0],
                Some(ColorChannel::Green) => pixel[0],
                Some(ColorChannel::Blue) => pixel[0],
                None => 255,
            };

            let metallic = match self.metallic_channel {
                Some(ColorChannel::Red) => pixel[0],
                Some(ColorChannel::Green) => pixel[0],
                Some(ColorChannel::Blue) => pixel[0],
                None => 255,
            };

            *pixel = image::Rgba([0, roughness, metallic, 255]);
        }

        image.into()
    }
}

/// Describes a texture
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TextureDesc {
    Path(String),
    Content(Asset<DynamicImage>),
    Color(image::Rgba<u8>),
    Processed(Box<ProcessedTexture>),
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

    pub fn process(self, processor: impl Into<StaticTextureProcessor>) -> Self {
        Self::Processed(Box::new(ProcessedTexture {
            texture: self,
            processor: processor.into(),
        }))
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

    pub fn label(&self) -> &str {
        match self {
            TextureDesc::Path(v) => v,
            TextureDesc::Content(_) => "content",
            TextureDesc::Color(_) => "color",
            TextureDesc::Processed(v) => v.texture.label(),
        }
    }

    fn load_image(&self, assets: &ivy_assets::AssetCache) -> anyhow::Result<DynamicImage> {
        match self {
            TextureDesc::Path(v) => v
                .try_load(assets)
                .map(|v: Asset<DynamicImage>| (*v).to_owned()),
            TextureDesc::Content(v) => Ok((**v).clone()),
            TextureDesc::Color(v) => Ok(ImageBuffer::from_pixel(32, 32, *v).into()),
            TextureDesc::Processed(v) => {
                let original = v.texture.load_image(assets)?;
                let processed = v.processor.process(original);
                Ok(processed)
            }
        }
    }

    async fn load_image_async(
        &self,
        assets: &ivy_assets::AssetCache,
    ) -> anyhow::Result<DynamicImage> {
        match self {
            TextureDesc::Path(v) => v
                .load_async(assets)
                .await
                .map(|v: Asset<DynamicImage>| (*v).to_owned())
                .map_err(Into::into),
            TextureDesc::Content(v) => Ok((**v).clone()),
            TextureDesc::Color(v) => Ok(ImageBuffer::from_pixel(32, 32, *v).into()),
            TextureDesc::Processed(v) => {
                let original = v.texture.load_image(assets)?;
                let processed = v.processor.process(original);
                Ok(processed)
            }
        }
    }
}

impl AssetDesc<DynamicImage> for TextureDesc {
    type Error = anyhow::Error;

    fn create(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<DynamicImage>, Self::Error> {
        match self {
            TextureDesc::Path(v) => v.try_load(assets),
            TextureDesc::Content(v) => Ok(v.clone()),
            TextureDesc::Color(v) => Ok(assets.insert(ImageBuffer::from_pixel(32, 32, *v).into())),
            TextureDesc::Processed(v) => {
                let original = v.texture.load_image(assets)?;
                let processed = v.processor.process(original);
                Ok(assets.insert(processed))
            }
        }
    }
}

impl AsyncAssetDesc<DynamicImage> for TextureDesc {
    type Error = anyhow::Error;

    async fn create(
        &self,
        assets: &ivy_assets::AssetCache,
    ) -> Result<Asset<DynamicImage>, Self::Error> {
        match self {
            TextureDesc::Path(v) => v.load_async(assets).await.map_err(|v| v.into()),
            TextureDesc::Content(v) => Ok(v.clone()),
            TextureDesc::Color(v) => Ok(assets.insert(ImageBuffer::from_pixel(32, 32, *v).into())),
            TextureDesc::Processed(v) => {
                let original = v.texture.load_image_async(assets).await?;
                let processed = v.processor.process(original);
                Ok(assets.insert(processed))
            }
        }
    }
}
