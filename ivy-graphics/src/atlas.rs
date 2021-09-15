use crate::{Error, Result};
use ash::vk;
use ivy_image::Image;
use ivy_resources::{Handle, Resources};
use ivy_vulkan::{Extent, Texture, TextureInfo, VulkanContext};
use rectangle_pack::{
    contains_smallest_box, volume_heuristic, GroupedRectsToPlace, RectToInsert, RectanglePackOk,
    TargetBin,
};
use std::collections::BTreeMap;
use std::hash::Hash;
use std::sync::Arc;

pub type BinId = ();
pub use rectangle_pack::PackedLocation;

pub struct NormalizedLocation {
    x: f32,
    y: f32,
    width: f32,
    height: f32,
}

impl NormalizedLocation {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
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
    ) -> Result<Self> {
        let mut packer = GroupedRectsToPlace::<K, BinId>::new();
        let extent = texture_info.extent;

        // Inserts rects and gather all pixels
        images.iter().for_each(|(k, image)| {
            assert_eq!(image.channels(), channels);

            packer.push_rect(
                k.clone(),
                None,
                RectToInsert::new(image.width(), image.height(), 1),
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
            let width = location.1.width() as usize;
            let height = location.1.height() as usize;

            dbg!(x, y, width, height);

            assert_eq!(width, image.width() as usize);
            assert_eq!(height, image.height() as usize);

            let image_pixels = image.pixels();

            (0..height).for_each(|row| unsafe {
                dbg!("Copying row", row);
                std::ptr::copy_nonoverlapping(
                    &image_pixels[width * row * stride] as *const u8,
                    &mut pixels[(extent.width as usize * (row + y) + x) * stride] as *mut u8,
                    width * stride,
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

        println!(
            "Pixels: {:?}, extent: {:?}",
            pixels.len(),
            texture_info.extent
        );

        texture.write(&pixels)?;

        let texture = resources.insert(texture)?;

        Ok(Self {
            rects,
            extent,
            texture,
        })
    }

    /// Returns the atlas extent.
    pub fn extent(&self) -> Extent {
        self.extent
    }

    /// Returns the unnormalized location of an image in the atlas.
    pub fn get(&self, key: &K) -> Result<PackedLocation> {
        self.rects
            .packed_locations()
            .get(key)
            .map(|val| val.1)
            .ok_or(Error::InvalidAtlasKey)
    }

    /// Returns the unnormalized location of an image in the atlas in 0..1
    /// coordinate space.
    pub fn get_normalized(&self, key: &K) -> Result<NormalizedLocation> {
        let extent = self.extent;
        let width = extent.width as f32;
        let height = extent.height as f32;

        self.get(key).map(|val| {
            NormalizedLocation::new(
                val.x() as f32 / width,
                val.y() as f32 / height,
                val.width() as f32 / width,
                val.height() as f32 / height,
            )
        })
    }

    /// Get a reference to the texture atlas's texture.
    pub fn texture(&self) -> Handle<Texture> {
        self.texture
    }
}
