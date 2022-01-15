use crate::context::SharedVulkanContext;
use crate::descriptors::DescriptorBindable;
use crate::traits::FromExtent;
use crate::{buffer, commands::*, Error, Result};
use crate::{Buffer, BufferAccess};
use ash::vk::{Extent3D, ImageAspectFlags, ImageView, SharingMode};
use gpu_allocator::vulkan::{Allocation, AllocationCreateDesc};
use gpu_allocator::MemoryLocation;
use ivy_base::Extent;
use ivy_resources::LoadResource;
use std::borrow::Cow;
use std::ops::Deref;
use std::path::Path;

use ash::vk;

pub use vk::Format;
pub use vk::ImageLayout;
pub use vk::ImageUsageFlags as ImageUsage;
pub use vk::SampleCountFlags;

/// Specifies texture creation info.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextureInfo {
    pub extent: Extent,
    /// The maximum amount of mip levels to use.
    /// Actual value may be lower due to texture size.
    /// A value of zero uses the maximum mip levels.
    /// NOTE: Multisampled images cannot use more than one miplevel
    pub mip_levels: u32,
    /// The type/aspect of texture.
    pub usage: ImageUsage,
    /// The pixel format.
    pub format: Format,
    pub samples: SampleCountFlags,
}

impl TextureInfo {
    /// Returns a texture info most suitable for a sampled color texture with
    /// the supplied extent using mipmapping.
    pub fn color(extent: Extent) -> Self {
        TextureInfo {
            extent,
            usage: ImageUsage::TRANSFER_DST | ImageUsage::TRANSFER_SRC | ImageUsage::SAMPLED,
            mip_levels: calculate_mip_levels(extent),
            ..Default::default()
        }
    }

    /// Creates a texture suitable for depth attachment
    pub fn depth(extent: Extent) -> Self {
        TextureInfo {
            extent,
            mip_levels: 1,
            usage: ImageUsage::DEPTH_STENCIL_ATTACHMENT
                | ImageUsage::SAMPLED
                | ImageUsage::INPUT_ATTACHMENT,
            format: Format::D32_SFLOAT,
            samples: SampleCountFlags::TYPE_1,
        }
    }
}

impl Default for TextureInfo {
    fn default() -> Self {
        Self {
            extent: (512, 512).into(),
            mip_levels: 1,
            usage: ImageUsage::SAMPLED,
            format: Format::R8G8B8A8_SRGB,
            samples: SampleCountFlags::TYPE_1,
        }
    }
}

// Represents a texture combining an image and image view. A texture also stores its own width,
// height, format, mipmapping levels and samples. Manages the deallocation of image memory unless
// created manually without provided allocation using `from_image`.
pub struct Texture {
    context: SharedVulkanContext,
    image: vk::Image,
    image_view: vk::ImageView,
    format: vk::Format,
    // May not necessarily own the allocation
    allocation: Option<Allocation>,
    extent: Extent,
    mip_levels: u32,
    samples: vk::SampleCountFlags,
    usage: ImageUsage,
}

impl Texture {
    /// Loads a color texture from an image in memory.
    /// Uses the width and height of the loaded image, no resizing.
    /// Uses mipmapping.
    pub fn from_memory(context: SharedVulkanContext, data: &[u8]) -> Result<Self> {
        let image = ivy_image::Image::load_from_memory(data, 4)?;

        let extent = (image.width(), image.height()).into();

        let texture = Self::new(context, &TextureInfo::color(extent))?;

        let size = image.width() as u64 * image.height() as u64 * 4;

        assert_eq!(size, image.pixels().len() as _);

        texture.write(image.pixels())?;
        Ok(texture)
    }
    /// Loads a color texture from an image file.
    /// Uses the width and height of the loaded image, no resizing.
    /// Uses mipmapping.
    pub fn load<P: AsRef<Path>>(context: SharedVulkanContext, path: P) -> Result<Self> {
        let image = ivy_image::Image::load(&path, 4)?;

        let extent = (image.width(), image.height()).into();

        let texture = Self::new(context, &TextureInfo::color(extent))?;

        let size = image.width() as u64 * image.height() as u64 * 4;

        assert_eq!(size, image.pixels().len() as _);

        texture.write(image.pixels())?;
        Ok(texture)
    }

    /// Creates a new unitialized texture
    /// Note, raw pixels must match format, width, and height
    pub fn new(context: SharedVulkanContext, info: &TextureInfo) -> Result<Self> {
        let mut mip_levels = calculate_mip_levels(info.extent);

        // Don't use more mip_levels than info
        if info.mip_levels != 0 {
            mip_levels = mip_levels.min(info.mip_levels)
        }

        let location = MemoryLocation::GpuOnly;

        let image_info = vk::ImageCreateInfo {
            image_type: vk::ImageType::TYPE_2D,
            format: info.format,
            extent: Extent3D::from_extent(info.extent),
            mip_levels,
            array_layers: 1,
            samples: info.samples,
            tiling: vk::ImageTiling::OPTIMAL,
            usage: info.usage,
            sharing_mode: SharingMode::EXCLUSIVE,
            initial_layout: vk::ImageLayout::UNDEFINED,
            ..Default::default()
        };

        let allocator = context.allocator();

        let device = context.device();
        let image = unsafe { device.create_image(&image_info, None)? };

        let requirements = unsafe { device.get_image_memory_requirements(image) };

        let allocation = allocator.write().allocate(&AllocationCreateDesc {
            name: "Image",
            requirements,
            location,
            linear: false,
        })?;

        unsafe { device.bind_image_memory(image, allocation.memory(), allocation.offset())? };

        Self::from_image(context, info, image, Some(allocation))
    }

    /// Creates a texture from an already existing VkImage
    /// If allocation is provided, the image will be destroyed along with self
    pub fn from_image(
        context: SharedVulkanContext,
        info: &TextureInfo,
        image: vk::Image,
        allocation: Option<Allocation>,
    ) -> Result<Self> {
        let aspect_mask = if info.usage.contains(ImageUsage::DEPTH_STENCIL_ATTACHMENT) {
            ImageAspectFlags::DEPTH
        } else {
            vk::ImageAspectFlags::COLOR
        };

        let create_info = vk::ImageViewCreateInfo::builder()
            .image(image)
            .view_type(vk::ImageViewType::TYPE_2D)
            .format(info.format)
            .subresource_range(vk::ImageSubresourceRange {
                aspect_mask,
                base_mip_level: 0,
                level_count: info.mip_levels,
                base_array_layer: 0,
                layer_count: 1,
            });

        let image_view = unsafe { context.device().create_image_view(&create_info, None) }?;

        Ok(Self {
            context,
            image,
            image_view,
            extent: info.extent,
            mip_levels: info.mip_levels,
            format: info.format,
            samples: info.samples,
            usage: info.usage,
            allocation,
        })
    }

    pub fn write(&self, pixels: &[u8]) -> Result<()> {
        // Create a new or reuse staging buffer
        let staging = Buffer::new(
            self.context.clone(),
            vk::BufferUsageFlags::TRANSFER_SRC,
            BufferAccess::Mapped,
            pixels,
        )?;

        let transfer_pool = self.context.transfer_pool();
        let graphics_queue = self.context.graphics_queue();

        // Prepare the image layout
        transition_layout(
            transfer_pool,
            graphics_queue,
            self.image,
            self.mip_levels,
            vk::ImageLayout::UNDEFINED,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
        )?;

        buffer::copy_to_image(
            transfer_pool,
            graphics_queue,
            staging.buffer(),
            self.image,
            vk::ImageLayout::TRANSFER_DST_OPTIMAL,
            self.extent,
        )?;

        // Generate Mipmaps
        generate_mipmaps(
            transfer_pool,
            graphics_queue,
            self.image,
            self.extent,
            self.mip_levels,
        )?;

        Ok(())
    }

    pub fn format(&self) -> vk::Format {
        self.format
    }

    pub fn image(&self) -> vk::Image {
        self.image
    }

    pub fn image_view(&self) -> vk::ImageView {
        self.image_view
    }

    pub fn mip_levels(&self) -> u32 {
        self.mip_levels
    }

    /// Return a reference to the texture's samples.
    pub fn samples(&self) -> vk::SampleCountFlags {
        self.samples
    }

    /// Return a reference to the texture's type
    pub fn usage(&self) -> ImageUsage {
        self.usage
    }

    // Returns the textures width and height
    pub fn extent(&self) -> Extent {
        self.extent
    }
}

impl AsRef<vk::ImageView> for Texture {
    fn as_ref(&self) -> &vk::ImageView {
        &self.image_view
    }
}

impl AsRef<vk::Image> for Texture {
    fn as_ref(&self) -> &vk::Image {
        &self.image
    }
}

impl From<&Texture> for vk::ImageView {
    fn from(val: &Texture) -> Self {
        val.image_view
    }
}

impl From<&Texture> for vk::Image {
    fn from(val: &Texture) -> Self {
        val.image
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        let allocator = self.context.allocator();

        let device = self.context.device();

        // Destroy allocation if texture owns image
        if let Some(allocation) = self.allocation.take() {
            allocator.write().free(allocation).unwrap();
            unsafe { device.destroy_image(self.image(), None) };
        }

        // Destroy image view
        unsafe {
            device.destroy_image_view(self.image_view, None);
        }
    }
}

fn calculate_mip_levels(extent: Extent) -> u32 {
    (extent.width.max(extent.height) as f32).log2().floor() as u32 + 1
}

fn generate_mipmaps(
    commandpool: &CommandPool,
    queue: vk::Queue,
    image: vk::Image,
    extent: Extent,
    mip_levels: u32,
) -> Result<()> {
    let mut barrier = vk::ImageMemoryBarrier {
        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
        p_next: std::ptr::null(),
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        image,
        subresource_range: vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: 1,
            base_array_layer: 0,
            layer_count: 1,
        },
        ..Default::default()
    };

    let mut mip_width = extent.width;
    let mut mip_height = extent.height;

    commandpool.single_time_command(queue, |commandbuffer| {
        for i in 1..mip_levels {
            barrier.subresource_range.base_mip_level = i - 1;
            barrier.old_layout = vk::ImageLayout::TRANSFER_DST_OPTIMAL;
            barrier.new_layout = vk::ImageLayout::TRANSFER_SRC_OPTIMAL;

            barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
            barrier.dst_access_mask = vk::AccessFlags::TRANSFER_READ;

            commandbuffer.pipeline_barrier(
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::TRANSFER,
                &[],
                &[barrier],
            );

            let offset = vk::Offset3D {
                x: if mip_width > 1 {
                    (mip_width / 2) as _
                } else {
                    1
                },
                y: if mip_height > 1 {
                    (mip_height / 2) as _
                } else {
                    1
                },
                z: 1,
            };

            let blit = vk::ImageBlit {
                src_offsets: [
                    vk::Offset3D { x: 0, y: 0, z: 0 },
                    vk::Offset3D {
                        x: mip_width as i32,
                        y: mip_height as i32,
                        z: 1,
                    },
                ],
                dst_offsets: [vk::Offset3D { x: 0, y: 0, z: 0 }, offset],
                src_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: i - 1,
                    base_array_layer: 0,
                    layer_count: 1,
                },
                dst_subresource: vk::ImageSubresourceLayers {
                    aspect_mask: vk::ImageAspectFlags::COLOR,
                    mip_level: i,
                    base_array_layer: 0,
                    layer_count: 1,
                },
            };

            commandbuffer.blit_image(
                image,
                vk::ImageLayout::TRANSFER_SRC_OPTIMAL,
                image,
                vk::ImageLayout::TRANSFER_DST_OPTIMAL,
                &[blit],
                vk::Filter::LINEAR,
            );

            // Transition new mip level to SHADER_READ_ONLY_OPTIMAL
            barrier.old_layout = vk::ImageLayout::TRANSFER_SRC_OPTIMAL;
            barrier.new_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
            barrier.src_access_mask = vk::AccessFlags::TRANSFER_READ;
            barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

            commandbuffer.pipeline_barrier(
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
                &[],
                &[barrier],
            );

            if mip_width > 1 {
                mip_width /= 2;
            }

            if mip_height > 1 {
                mip_height /= 2;
            }
        }

        // Transition the last mip level to SHADER_READ_ONLY_OPTIMAL
        barrier.subresource_range.base_mip_level = mip_levels - 1;
        barrier.old_layout = vk::ImageLayout::TRANSFER_DST_OPTIMAL;
        barrier.new_layout = vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL;
        barrier.src_access_mask = vk::AccessFlags::TRANSFER_WRITE;
        barrier.dst_access_mask = vk::AccessFlags::SHADER_READ;

        commandbuffer.pipeline_barrier(
            vk::PipelineStageFlags::TRANSFER,
            vk::PipelineStageFlags::FRAGMENT_SHADER,
            &[],
            &[barrier],
        );
    })
}

// Transitions image layout from one layout to another using a pipeline barrier
fn transition_layout(
    commandpool: &CommandPool,
    queue: vk::Queue,
    image: vk::Image,
    mip_levels: u32,
    old_layout: vk::ImageLayout,
    new_layout: vk::ImageLayout,
) -> Result<()> {
    let (src_access_mask, dst_access_mask, src_stage_mask, dst_stage_mask) =
        match (old_layout, new_layout) {
            (vk::ImageLayout::UNDEFINED, vk::ImageLayout::TRANSFER_DST_OPTIMAL) => (
                vk::AccessFlags::default(),
                vk::AccessFlags::TRANSFER_WRITE,
                vk::PipelineStageFlags::TOP_OF_PIPE,
                vk::PipelineStageFlags::TRANSFER,
            ),

            (vk::ImageLayout::TRANSFER_DST_OPTIMAL, vk::ImageLayout::SHADER_READ_ONLY_OPTIMAL) => (
                vk::AccessFlags::TRANSFER_WRITE,
                vk::AccessFlags::SHADER_READ,
                vk::PipelineStageFlags::TRANSFER,
                vk::PipelineStageFlags::FRAGMENT_SHADER,
            ),
            _ => return Err(Error::UnsupportedLayoutTransition(old_layout, new_layout)),
        };

    let barrier = vk::ImageMemoryBarrier {
        s_type: vk::StructureType::IMAGE_MEMORY_BARRIER,
        p_next: std::ptr::null(),
        src_access_mask,
        dst_access_mask,
        old_layout,
        new_layout,
        src_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        dst_queue_family_index: vk::QUEUE_FAMILY_IGNORED,
        image,
        subresource_range: vk::ImageSubresourceRange {
            aspect_mask: vk::ImageAspectFlags::COLOR,
            base_mip_level: 0,
            level_count: mip_levels,
            base_array_layer: 0,
            layer_count: 1,
        },
    };

    commandpool.single_time_command(queue, |commandbuffer| {
        commandbuffer.pipeline_barrier(src_stage_mask, dst_stage_mask, &[], &[barrier])
    })
}

impl DescriptorBindable for Texture {
    fn bind_resource<'a>(
        &self,
        binding: u32,
        stage: vk::ShaderStageFlags,
        builder: &'a mut crate::descriptors::DescriptorBuilder,
    ) -> Result<&'a mut crate::descriptors::DescriptorBuilder> {
        Ok(builder.bind_image(binding, stage, self))
    }
}

pub struct InputAttachment {
    pub image: ImageView,
}

impl InputAttachment {
    pub fn new<T: AsRef<ImageView>>(texture: T) -> Self {
        Self {
            image: *texture.as_ref(),
        }
    }
}

impl From<InputAttachment> for ImageView {
    fn from(val: InputAttachment) -> Self {
        val.image
    }
}

impl AsRef<ImageView> for InputAttachment {
    fn as_ref(&self) -> &ImageView {
        &self.image
    }
}

impl DescriptorBindable for InputAttachment {
    fn bind_resource<'a>(
        &self,
        binding: u32,
        stage: vk::ShaderStageFlags,
        builder: &'a mut crate::descriptors::DescriptorBuilder,
    ) -> Result<&'a mut crate::descriptors::DescriptorBuilder> {
        Ok(builder.bind_input_attachment(binding, stage, self.image))
    }
}

impl Deref for InputAttachment {
    type Target = ImageView;

    fn deref(&self) -> &ImageView {
        &self.image
    }
}

pub struct CombinedImageSampler {
    pub image: ImageView,
    pub sampler: vk::Sampler,
}

impl CombinedImageSampler {
    pub fn new<T: AsRef<ImageView>, S: AsRef<vk::Sampler>>(texture: T, sampler: S) -> Self {
        Self {
            image: *texture.as_ref(),
            sampler: *sampler.as_ref(),
        }
    }

    /// Get the combined image sampler's image.
    pub fn image(&self) -> ImageView {
        self.image
    }

    /// Get the combined image sampler's sampler.
    pub fn sampler(&self) -> vk::Sampler {
        self.sampler
    }
}
impl DescriptorBindable for CombinedImageSampler {
    fn bind_resource<'a>(
        &self,
        binding: u32,
        stage: vk::ShaderStageFlags,
        builder: &'a mut crate::descriptors::DescriptorBuilder,
    ) -> Result<&'a mut crate::descriptors::DescriptorBuilder> {
        Ok(builder.bind_combined_image_sampler(binding, stage, self.image, self.sampler))
    }
}

impl LoadResource for Texture {
    type Info = Cow<'static, str>;

    type Error = Error;

    fn load(resources: &ivy_resources::Resources, path: &Self::Info) -> Result<Self> {
        let context = resources.get_default::<SharedVulkanContext>()?;
        Self::load(context.clone(), path.as_ref())
    }
}
