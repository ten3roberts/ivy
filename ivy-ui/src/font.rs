use crate::{Error, Result};
use fontdue::Metrics;
use ivy_graphics::{PackedLocation, TextureAtlas};
use ivy_resources::Resources;
use ivy_vulkan::{Extent, Format, ImageUsage, SampleCountFlags, TextureInfo, VulkanContext};
use std::{collections::BTreeMap, ops::Range, path::Path, sync::Arc};

pub struct FontInfo {
    // Font size
    pub size: f32,
    pub glyphs: Range<char>,
}

pub struct Font {
    atlas: TextureAtlas<char>,
    metrics: BTreeMap<char, Metrics>,
}

impl Font {
    /// Loads and rasterizes a font from file
    pub fn new<P: AsRef<Path>>(
        context: Arc<VulkanContext>,
        resources: &Resources,
        path: P,
        info: &FontInfo,
    ) -> Result<Self> {
        let path = path.as_ref();

        let bytes = std::fs::read(path).map_err(|e| Error::Io(e, Some(path.to_owned())))?;

        let font = fontdue::Font::from_bytes(&bytes[..], fontdue::FontSettings::default())
            .map_err(|e| Error::FontParsing(e))?;

        let glyphs = Self::rasterize(&font, &info);

        let metrics = glyphs.0.iter().cloned().collect::<BTreeMap<_, _>>();
        let images = glyphs.1;

        let dimension = (((info.glyphs.end as usize - info.glyphs.start as usize) as f32)
            .sqrt()
            .ceil()
            * info.size) as u32;

        let atlas = TextureAtlas::new(
            context,
            resources,
            &TextureInfo {
                extent: Extent::new(dimension, dimension),
                mip_levels: 1,
                usage: ImageUsage::SAMPLED,
                format: Format::R8_SRGB,
                samples: SampleCountFlags::TYPE_1,
            },
            1,
            images,
        )?;

        dbg!("Metrics", &metrics);

        Ok(Self { atlas, metrics })
    }

    fn rasterize(
        font: &fontdue::Font,
        info: &FontInfo,
    ) -> (Vec<(char, Metrics)>, Vec<(char, ivy_image::Image)>) {
        info.glyphs
            .clone()
            .map(|c| {
                let (metrics, pixels) = font.rasterize(c, info.size);
                dbg!("Pixels: ", &pixels);
                let image = ivy_image::Image::new(
                    metrics.width as _,
                    metrics.height as _,
                    1,
                    pixels.into_boxed_slice(),
                );

                ((c, metrics), (c, image))
            })
            .unzip()
    }

    /// Get a reference to the font's atlas.
    pub fn atlas(&self) -> &TextureAtlas<char> {
        &self.atlas
    }

    pub fn get(&self, glyph: char) -> Result<(&Metrics, PackedLocation)> {
        Ok((
            self.metrics()
                .get(&glyph)
                .ok_or(Error::MissingGlyph(glyph))?,
            self.atlas
                .get(&glyph)
                .map_err(|_| Error::MissingGlyph(glyph))?,
        ))
    }

    /// Get a reference to the font's metrics.
    pub fn metrics(&self) -> &BTreeMap<char, Metrics> {
        &self.metrics
    }
}
