use image::{DynamicImage, ImageBuffer};
use ivy_assets::{Asset, AssetDesc};
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
    Content(Asset<DynamicImage>),
    Color(image::Rgba<u8>),
    Processed(Box<ProcessedTexture>),
}

impl TextureDesc {
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

    pub fn label(&self) -> String {
        match self {
            TextureDesc::Content(_) => "content".to_string(),
            TextureDesc::Color(_) => "color".to_string(),
            TextureDesc::Processed(v) => v.texture.label().to_string(),
        }
    }

    fn load_image(&self) -> anyhow::Result<DynamicImage> {
        match self {
            TextureDesc::Content(v) => Ok((**v).clone()),
            TextureDesc::Color(v) => Ok(ImageBuffer::from_pixel(32, 32, *v).into()),
            TextureDesc::Processed(v) => {
                let original = v.texture.load_image()?;
                let processed = v.processor.process(original);
                Ok(processed)
            }
        }
    }
}

impl AssetDesc<DynamicImage> for TextureDesc {
    type Error = anyhow::Error;

    fn create(&self, assets: &ivy_assets::AssetCache) -> Result<Asset<DynamicImage>, Self::Error> {
        Ok(assets.insert(self.load_image()?))
    }
}
