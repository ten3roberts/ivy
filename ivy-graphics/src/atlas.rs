use crate::{Error, Result};
use ash::vk;
use ivy_image::Image;
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{Extent, Texture, TextureInfo, VulkanContext};
use rectangle_pack::{
    contains_smallest_box, volume_heuristic, GroupedRectsToPlace, PackedLocation, RectToInsert,
    RectanglePackOk, TargetBin,
};
use std::collections::BTreeMap;
use std::hash::Hash;
use std::sync::Arc;

pub type BinId = ();

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    // Offset to the right of the bitmap
    pub x: u32,
    // Offset from the top of the bitmap
    pub y: u32,
    pub z: u32,
    pub extent: Extent,
    // Usually the amount of channels
    pub depth: u32,
}

impl From<PackedLocation> for Rect {
    fn from(val: PackedLocation) -> Self {
        Self {
            x: val.x(),
            y: val.y(),
            z: val.z(),
            depth: val.depth(),
            extent: Extent::new(val.width(), val.height()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct NormalizedRect {
    pub x: f32,
    pub y: f32,
    pub z: u32,
    pub width: f32,
    pub height: f32,
    pub depth: u32,
}

impl NormalizedRect {
    pub fn new(x: f32, y: f32, z: u32, width: f32, height: f32, depth: u32) -> Self {
        Self {
            x,
            y,
            z,
            width,
            height,
            depth,
        }
    }

    /// Get the normalized location's x.
    pub fn x(&self) -> f32 {
        self.x
    }

    /// Get the normalized location's y.
    pub fn y(&self) -> f32 {
        self.y
    }

    /// Get the normalized location's width.
    pub fn width(&self) -> f32 {
        self.width
    }

    /// Get the normalized location's height.
    pub fn height(&self) -> f32 {
        self.height
    }
}

pub struct TextureAtlas<K: AtlasKey> {
    rects: RectanglePackOk<K, BinId>,
    extent: Extent,
    texture: Handle<Texture>,
    padding: u32,
}

pub trait AtlasKey: Hash + std::fmt::Debug + PartialEq + Eq + PartialOrd + Ord + Clone {}

impl<T> AtlasKey for T where T: Hash + std::fmt::Debug + PartialEq + Eq + PartialOrd + Ord + Clone {}

impl<K> TextureAtlas<K>
where
    K: AtlasKey,
{
    /// Creates a new texture atlas of `extent`. All images will attempt to be
    /// packed. All images are expected to have the same number of channels.
    pub fn new(
        context: Arc<VulkanContext>,
        resources: &Resources,
        texture_info: &TextureInfo,
        channels: u32,
        images: Vec<(K, Image)>,
        padding: u32,
    ) -> Result<Self> {
        dbg!("Creating atlas");
        let mut packer = GroupedRectsToPlace::<K, BinId>::new();
        let extent = texture_info.extent;

        // Inserts rects and gather all pixels
        images.iter().for_each(|(k, image)| {
            assert_eq!(image.channels(), channels);

            packer.push_rect(
                k.clone(),
                None,
                RectToInsert::new(image.width() + padding, image.height() + padding, 1),
            );
        });

        let mut bins = BTreeMap::new();
        bins.insert(
            BinId::default(),
            TargetBin::new(extent.width, extent.height, 1),
        );

        let rects = rectangle_pack::pack_rects(
            &packer,
            &mut bins,
            &volume_heuristic,
            &contains_smallest_box,
        )
        .map_err(|_| Error::RectanglePack(extent))?;

        // Create texture
        let mut pixels = Vec::new();
        pixels.extend(
            std::iter::repeat(0_u8)
                .take(extent.width as usize * extent.height as usize * channels as usize),
        );

        let locations = rects.packed_locations();
        let stride = channels as usize;

        // Copy all images into their rect in the atlas
        images.iter().for_each(|(k, image)| {
            // Copy each row
            let location = locations[k];
            let x = location.1.x() as usize;
            let y = location.1.y() as usize;
            let img_width = image.width() as usize;
            let img_height = image.height() as usize;

            // assert_eq!(width, image.width() as usize);
            // assert_eq!(height, image.height() as usize);

            let image_pixels = image.pixels();

            (0..img_height).for_each(|row| unsafe {
                std::ptr::copy_nonoverlapping(
                    &image_pixels[img_width * row * stride] as *const u8,
                    &mut pixels[(extent.width as usize * (row + y) + x) * stride] as *mut u8,
                    img_width * stride,
                )
            })
        });

        let texture = Texture::new(
            context,
            &TextureInfo {
                extent,
                mip_levels: 1,
                samples: vk::SampleCountFlags::TYPE_1,
                ..*texture_info
            },
        )?;

        texture.write(&pixels)?;

        let texture = resources.insert(texture)?;

        Ok(Self {
            rects,
            extent,
            texture,
            padding,
        })
    }

    /// Returns the atlas extent.
    pub fn extent(&self) -> Extent {
        self.extent
    }

    /// Returns the unnormalized location of an image in the atlas.
    pub fn get(&self, key: &K) -> Result<Rect> {
        self.rects
            .packed_locations()
            .get(key)
            .map(|val| {
                let mut rect: Rect = val.1.into();
                rect.extent.width -= self.padding;
                rect.extent.height -= self.padding;
                rect
            })
            .ok_or(Error::InvalidAtlasKey)
    }

    /// Returns the unnormalized location of an image in the atlas in 0..1
    /// coordinate space.
    pub fn get_normalized(&self, key: &K) -> Result<NormalizedRect> {
        let extent = self.extent;
        let width = extent.width as f32;
        let height = extent.height as f32;

        self.get(key).map(|val| {
            NormalizedRect::new(
                val.x as f32 / width,
                val.y as f32 / height,
                val.z,
                val.extent.width as f32 / width,
                val.extent.height as f32 / height,
                val.depth,
            )
        })
    }

    /// Get a reference to the texture atlas's texture.
    pub fn texture(&self) -> Handle<Texture> {
        self.texture
    }
}
