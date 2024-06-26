use crate::{Error, Result};
use ivy_assets::{Asset, AssetCache, AssetKey};
use ivy_base::Extent;
use ivy_graphics::{NormalizedRect, Rect, TextureAtlas};
use ivy_vulkan::{
    context::{SharedVulkanContext, VulkanContextService},
    descriptors::{DescriptorBuilder, DescriptorSet, IntoSet},
    vk::ShaderStageFlags,
    AddressMode, FilterMode, Format, ImageUsage, SampleCountFlags, Sampler, SamplerKey,
    TextureInfo,
};
use std::borrow::Cow;

#[derive(PartialEq, Debug, Clone)]
pub struct FontInfo {
    // The most optimal pixel size for the rasterized font.
    pub size: f32,
    pub glyphs: Vec<char>,
    pub padding: u32,
    pub mip_levels: u32,
    pub path: Cow<'static, str>,
}

impl Eq for FontInfo {}

impl std::hash::Hash for FontInfo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        ((self.size * 100.0) as u32).hash(state);
        self.glyphs.hash(state);
        self.padding.hash(state);
        self.mip_levels.hash(state);
    }
}

impl Default for FontInfo {
    fn default() -> Self {
        Self {
            size: 36.0,
            glyphs: (32..128)
                .map(|v| char::from_u32(v).unwrap())
                .into_iter()
                .chain(['å', 'ä', 'ö'])
                .collect(),
            padding: 5,
            mip_levels: 1,
            path: "".into(),
        }
    }
}

pub struct Font {
    atlas: TextureAtlas<u16>,
    size: f32,
    font: fontdue::Font,
    set: DescriptorSet,
}

impl Font {
    /// Loads and rasterizes a font from file
    pub fn new(
        context: SharedVulkanContext,
        assets: &AssetCache,
        sampler: Asset<Sampler>,
        info: &FontInfo,
    ) -> Result<Self> {
        let bytes = std::fs::read(info.path.as_ref())
            .map_err(|e| Error::Io(e, Some(info.path.to_string().into())))?;

        let font = fontdue::Font::from_bytes(&bytes[..], fontdue::FontSettings::default())
            .map_err(|e| Error::FontParsing(e))?;

        let images = Self::rasterize(&font, &info);

        let glyph_count = info.glyphs.len();

        let height = font
            .vertical_line_metrics(info.size)
            .map(|val| val.new_line_size)
            .unwrap_or(info.size);

        let dimension = nearest_power_2(
            ((glyph_count as f32).sqrt() * (height + info.padding as f32)).ceil() as u32,
        );

        let atlas = TextureAtlas::new(
            context.clone(),
            assets,
            &TextureInfo {
                extent: Extent::new(dimension, dimension),
                mip_levels: info.mip_levels,
                usage: ImageUsage::SAMPLED | ImageUsage::TRANSFER_DST,
                format: Format::R8_UNORM,
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
                atlas.texture().image_view(),
                sampler.sampler(),
            )
            .build(&context)?;

        Ok(Self {
            atlas,
            font,
            size: info.size as f32,
            set,
        })
    }

    fn rasterize(font: &fontdue::Font, info: &FontInfo) -> Vec<(u16, ivy_image::Image)> {
        let size = info.size as f32;

        info.glyphs
            .iter()
            .cloned()
            .filter_map(|c| {
                let (metrics, pixels) = font.rasterize(c, size);

                let image = ivy_image::Image::new(
                    metrics.width as _,
                    metrics.height as _,
                    1,
                    pixels.into_boxed_slice(),
                );

                let idx = font.lookup_glyph_index(c) as u16;

                Some((idx, image))
            })
            .collect()
    }

    /// Get a reference to the font's atlas.
    pub fn atlas(&self) -> &TextureAtlas<u16> {
        &self.atlas
    }

    pub fn get(&self, glyph: u16) -> Result<Rect> {
        self.atlas
            .get(&glyph)
            .map_err(|_| Error::MissingGlyph(glyph))
    }

    pub fn get_normalized(&self, glyph: u16) -> Result<NormalizedRect> {
        self.atlas
            .get_normalized(&glyph)
            .map_err(|_| Error::MissingGlyph(glyph))
    }

    /// Returs the base glyph size.
    pub fn size(&self) -> f32 {
        self.size
    }

    /// Get a reference to the font's font.
    pub fn font(&self) -> &fontdue::Font {
        &self.font
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

impl AssetKey<Font> for FontInfo {
    type Error = Error;

    fn load(&self, assets: &AssetCache) -> Result<Asset<Font>> {
        let context = assets.service::<VulkanContextService>().context();

        let sampler = assets.load(&SamplerKey {
            address_mode: AddressMode::CLAMP_TO_BORDER,
            mag_filter: FilterMode::LINEAR,
            min_filter: FilterMode::LINEAR,
            unnormalized_coordinates: false,
            anisotropy: 0,
            mip_levels: 1,
        });

        Ok(assets.insert(Font::new(context.clone(), assets, sampler, self)?))
    }
}
