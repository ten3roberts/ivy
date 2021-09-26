use crate::{Error, Result};
use fontdue::Metrics;
use ivy_graphics::{NormalizedRect, Rect, TextureAtlas};
use ivy_resources::{Handle, LoadResource, Resources};
use ivy_vulkan::{
    descriptors::{DescriptorBuilder, DescriptorSet, IntoSet},
    vk::ShaderStageFlags,
    AddressMode, Extent, FilterMode, Format, ImageUsage, SampleCountFlags, Sampler, SamplerInfo,
    TextureInfo, VulkanContext,
};
use std::{borrow::Cow, collections::BTreeMap, ops::Range, path::Path, sync::Arc};

pub struct FontInfo {
    // The most optimal pixel size for the rasterized font.
    pub size: f32,
    pub glyphs: Range<char>,
    pub padding: u32,
    pub mip_levels: u32,
}

impl Default for FontInfo {
    fn default() -> Self {
        Self {
            size: 36.0,
            glyphs: 0 as char..128 as char,
            padding: 5,
            mip_levels: 1,
        }
    }
}

pub struct Font {
    atlas: TextureAtlas<char>,
    size: f32,
    metrics: BTreeMap<char, Metrics>,
    set: DescriptorSet,
}

impl Font {
    /// Loads and rasterizes a font from file
    pub fn new<P: AsRef<Path>>(
        context: Arc<VulkanContext>,
        resources: &Resources,
        path: P,
        sampler: Handle<Sampler>,
        info: &FontInfo,
    ) -> Result<Self> {
        let path = path.as_ref();

        let bytes = std::fs::read(path).map_err(|e| Error::Io(e, Some(path.to_owned())))?;

        let font = fontdue::Font::from_bytes(&bytes[..], fontdue::FontSettings::default())
            .map_err(|e| Error::FontParsing(e))?;

        let glyphs = Self::rasterize(&font, &info);

        let metrics = glyphs.1.iter().cloned().collect::<BTreeMap<_, _>>();
        let images = glyphs.2;

        let avg_width = glyphs.0;
        dbg!(avg_width);

        let dimension = nearest_power_2(
            (((info.glyphs.end as usize - info.glyphs.start as usize) as f32).sqrt()
                * (avg_width + info.padding as f32))
                .ceil() as u32,
        );

        let atlas = TextureAtlas::new(
            context.clone(),
            resources,
            &TextureInfo {
                extent: Extent::new(dimension, dimension),
                mip_levels: info.mip_levels,
                usage: ImageUsage::SAMPLED | ImageUsage::TRANSFER_DST,
                format: Format::R8_SRGB,
                samples: SampleCountFlags::TYPE_1,
            },
            1,
            images,
            info.padding,
        )?;

        let set = DescriptorBuilder::new()
            .bind_combined_image_sampler(
                0,
                ShaderStageFlags::FRAGMENT,
                resources.get(atlas.texture())?.image_view(),
                resources.get(sampler)?.sampler(),
            )
            .build(&context)?;

        Ok(Self {
            atlas,
            size: info.size,
            metrics,
            set,
        })
    }

    fn rasterize(
        font: &fontdue::Font,
        info: &FontInfo,
    ) -> (f32, Vec<(char, Metrics)>, Vec<(char, ivy_image::Image)>) {
        let mut max_width = 0;
        let mut glyph_count = 0;

        let (a, b) = info
            .glyphs
            .clone()
            .filter_map(|c| {
                let (metrics, pixels) = font.rasterize(c, info.size);

                max_width = metrics.width.max(max_width);

                glyph_count += 1;

                let image = ivy_image::Image::new(
                    metrics.width as _,
                    metrics.height as _,
                    1,
                    pixels.into_boxed_slice(),
                );

                Some(((c, metrics), (c, image)))
            })
            .unzip();

        (max_width as f32, a, b)
    }

    /// Get a reference to the font's atlas.
    pub fn atlas(&self) -> &TextureAtlas<char> {
        &self.atlas
    }

    pub fn get(&self, glyph: char) -> Result<(&Metrics, Rect)> {
        Ok((
            self.metrics()
                .get(&glyph)
                .ok_or(Error::MissingGlyph(glyph))?,
            self.atlas
                .get(&glyph)
                .map_err(|_| Error::MissingGlyph(glyph))?,
        ))
    }

    pub fn get_normalized(&self, glyph: char) -> Result<(&Metrics, NormalizedRect)> {
        Ok((
            self.metrics()
                .get(&glyph)
                .ok_or(Error::MissingGlyph(glyph))?,
            self.atlas
                .get_normalized(&glyph)
                .map_err(|_| Error::MissingGlyph(glyph))?,
        ))
    }
    /// Get a reference to the font's metrics.
    pub fn metrics(&self) -> &BTreeMap<char, Metrics> {
        &self.metrics
    }

    /// Returs the base glyph size.
    pub fn size(&self) -> f32 {
        self.size
    }
}

impl IntoSet for Font {
    fn set(&self, _: usize) -> DescriptorSet {
        self.set
    }

    fn sets(&self) -> &[DescriptorSet] {
        std::slice::from_ref(&self.set)
    }
}

fn nearest_power_2(val: u32) -> u32 {
    let mut result = 1;
    while result < val {
        result *= 2;
    }
    result
}

impl LoadResource for Font {
    type Info = (FontInfo, Cow<'static, str>);

    type Error = Error;

    fn load(resources: &Resources, info: &Self::Info) -> Result<Self> {
        let context = resources.get_default::<Arc<VulkanContext>>()?;

        let sampler = resources.load(SamplerInfo {
            address_mode: AddressMode::CLAMP_TO_BORDER,
            mag_filter: FilterMode::LINEAR,
            min_filter: FilterMode::LINEAR,
            unnormalized_coordinates: false,
            anisotropy: 0.0,
            mip_levels: 1,
        })??;

        Self::new(
            context.clone(),
            resources,
            info.1.as_ref(),
            sampler,
            &info.0,
        )
    }
}
