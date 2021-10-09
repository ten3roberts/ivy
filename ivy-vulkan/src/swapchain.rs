use crate::surface::Backend;
use crate::ImageUsage;
use crate::{Error, Extent, Result, VulkanContext};
use ash::extensions::khr::Surface;
pub use ash::extensions::khr::Swapchain as SwapchainLoader;
use ash::vk::{self, Image, SurfaceKHR};
use ash::Device;
use ash::Instance;
use std::{cmp, sync::Arc};

/// The maximum number of images in the swapchain. Actual image count may be less but never more.
/// This is to allow inline allocation of per swapchain image resources through `ArrayVec`.
pub const MAX_FRAMES: usize = 5;

/// Preferred swapchain create info.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SwapchainInfo {
    pub present_mode: vk::PresentModeKHR,
    pub format: vk::SurfaceFormatKHR,
    /// The preferred number of images in the swapchain
    pub image_count: u32,
    pub usage: ImageUsage,
}

impl Default for SwapchainInfo {
    fn default() -> Self {
        Self {
            present_mode: vk::PresentModeKHR::IMMEDIATE,
            format: vk::SurfaceFormatKHR {
                format: vk::Format::B8G8R8A8_SRGB,
                color_space: vk::ColorSpaceKHR::SRGB_NONLINEAR,
            },
            image_count: 2,
            usage: ImageUsage::TRANSFER_DST | ImageUsage::COLOR_ATTACHMENT | ImageUsage::SAMPLED,
        }
    }
}

#[derive(Debug)]
pub(crate) struct SwapchainSupport {
    pub capabilities: vk::SurfaceCapabilitiesKHR,
    pub formats: Vec<vk::SurfaceFormatKHR>,
    pub present_modes: Vec<vk::PresentModeKHR>,
}

pub(crate) fn query_support(
    surface_loader: &Surface,
    surface: SurfaceKHR,
    physical_device: vk::PhysicalDevice,
) -> Result<SwapchainSupport> {
    let capabilities = unsafe {
        surface_loader.get_physical_device_surface_capabilities(physical_device, surface)?
    };

    let formats =
        unsafe { surface_loader.get_physical_device_surface_formats(physical_device, surface)? };

    let present_modes = unsafe {
        surface_loader.get_physical_device_surface_present_modes(physical_device, surface)?
    };

    Ok(SwapchainSupport {
        capabilities,
        formats,
        present_modes,
    })
}

fn pick_format(
    formats: &[vk::SurfaceFormatKHR],
    preferred_format: vk::SurfaceFormatKHR,
) -> vk::SurfaceFormatKHR {
    for surface_format in formats {
        // Preferred surface_format
        if *surface_format == preferred_format {
            return *surface_format;
        }
    }

    formats[0]
}

/// Picks a present mode
/// If `preferred` is available, it is used
/// Otherwise, FIFO is returned
fn pick_present_mode(
    modes: &[vk::PresentModeKHR],
    preferred: vk::PresentModeKHR,
) -> vk::PresentModeKHR {
    for mode in modes {
        // Preferred surface_format
        if *mode == preferred {
            return *mode;
        }
    }

    vk::PresentModeKHR::FIFO
}

fn pick_extent<T: Backend>(window: &T, capabilities: &vk::SurfaceCapabilitiesKHR) -> Extent {
    // The extent of the surface needs to match exactly
    if capabilities.current_extent.width != std::u32::MAX {
        return capabilities.current_extent.into();
    }

    // Freely choose extent based on window and min-max capabilities
    let extent = window.framebuffer_size();

    let width = cmp::max(
        capabilities.min_image_extent.width,
        cmp::min(capabilities.max_image_extent.width, extent.width as u32),
    );

    let height = cmp::max(
        capabilities.min_image_extent.height,
        cmp::min(capabilities.max_image_extent.height, extent.height as u32),
    );

    (width, height).into()
}

/// Contains a queue of images and is the link between vulkan and presenting image data to the
/// system window.
pub struct Swapchain {
    context: Arc<VulkanContext>,
    swapchain: vk::SwapchainKHR,
    images: Vec<Image>,
    extent: Extent,
    // The currently acquired swapchain image
    image_index: Option<u32>,
    surface_format: vk::SurfaceFormatKHR,
}

impl Swapchain {
    pub fn new<T: Backend>(
        context: Arc<VulkanContext>,
        window: &T,
        info: SwapchainInfo,
    ) -> Result<Self> {
        let support = query_support(
            context.surface_loader(),
            context.surface().unwrap(),
            context.physical_device(),
        )?;

        // Use one more image than the minumum supported
        let mut image_count = info.image_count.max(support.capabilities.min_image_count);

        // Make sure max image count isn't exceeded
        if support.capabilities.max_image_count != 0 {
            image_count = cmp::min(image_count, support.capabilities.max_image_count);
        }

        // The full set
        let queue_family_indices = [
            context.queue_families().graphics().unwrap(),
            context.queue_families().present().unwrap(),
        ];

        // Decide sharing mode depending on if graphics == present
        let (sharing_mode, queue_family_indices): (vk::SharingMode, &[u32]) =
            if context.queue_families().graphics() == context.queue_families().present() {
                (vk::SharingMode::EXCLUSIVE, &[])
            } else {
                (vk::SharingMode::CONCURRENT, &queue_family_indices)
            };

        let surface_format = pick_format(&support.formats, info.format);

        let present_mode = pick_present_mode(&support.present_modes, info.present_mode);

        let extent = pick_extent(window, &support.capabilities);

        let create_info = vk::SwapchainCreateInfoKHR::builder()
            .surface(context.surface().unwrap())
            .min_image_count(image_count)
            .image_format(surface_format.format)
            .image_color_space(surface_format.color_space)
            .image_extent(extent.into())
            .image_array_layers(1)
            // For now, render directly to the images
            .image_usage(info.usage)
            .image_sharing_mode(sharing_mode)
            .queue_family_indices(queue_family_indices)
            .pre_transform(support.capabilities.current_transform)
            .composite_alpha(vk::CompositeAlphaFlagsKHR::OPAQUE)
            .present_mode(present_mode)
            .clipped(true)
            .old_swapchain(vk::SwapchainKHR::null());

        let swapchain_loader = context.swapchain_loader();
        let swapchain = unsafe { swapchain_loader.create_swapchain(&create_info, None)? };

        let images = unsafe { swapchain_loader.get_swapchain_images(swapchain)? };

        Ok(Swapchain {
            context,
            swapchain,
            images,
            extent,
            surface_format,
            image_index: None,
        })
    }

    // Returns the next available image in the swapchain. Remembers the acquired image index in
    // self. image index is set to None on failure
    pub fn acquire_next_image(&mut self, semaphore: vk::Semaphore) -> Result<u32> {
        self.image_index = None;
        let (image_index, _) = unsafe {
            self.context.swapchain_loader().acquire_next_image(
                self.swapchain,
                std::u64::MAX,
                semaphore,
                vk::Fence::null(),
            )?
        };

        self.image_index = Some(image_index);

        Ok(image_index)
    }

    // Presents the currently acquired swapchain image
    pub fn present(&self, queue: vk::Queue, wait_semaphores: &[vk::Semaphore]) -> Result<bool> {
        let present_info = vk::PresentInfoKHR {
            s_type: vk::StructureType::PRESENT_INFO_KHR,
            p_next: std::ptr::null(),
            wait_semaphore_count: wait_semaphores.len() as _,
            p_wait_semaphores: wait_semaphores.as_ptr(),
            swapchain_count: 1,
            p_swapchains: &self.swapchain,
            p_image_indices: &self.image_index()?,
            p_results: std::ptr::null_mut(),
        };
        let suboptimal = unsafe {
            self.context
                .swapchain_loader()
                .queue_present(queue, &present_info)?
        };

        Ok(suboptimal)
    }

    pub fn image_index(&self) -> Result<u32> {
        self.image_index.ok_or(Error::NoCurrentSwapchainImage)
    }

    /// Returns the number of image in the swapchain. The same as `color_attachments`.len()
    pub fn image_count(&self) -> usize {
        self.images.len()
    }

    pub fn image_format(&self) -> vk::Format {
        self.surface_format.format
    }

    pub fn surface_format(&self) -> vk::SurfaceFormatKHR {
        self.surface_format
    }

    pub fn extent(&self) -> Extent {
        self.extent
    }

    /// Get a reference to a swapchain image by index
    pub fn image(&self, index: usize) -> Image {
        self.images[index]
    }

    /// Get a reference to the swapchain's images
    pub fn images(&self) -> &Vec<Image> {
        &self.images
    }

    pub fn create_loader(instance: &Instance, device: &Device) -> SwapchainLoader {
        SwapchainLoader::new(instance, device)
    }

    /// Get a reference to the swapchain's context.
    pub fn context(&self) -> &Arc<VulkanContext> {
        &self.context
    }
}

impl Drop for Swapchain {
    fn drop(&mut self) {
        // Destroy the swapchain
        unsafe {
            self.context
                .swapchain_loader()
                .destroy_swapchain(self.swapchain, None);
        };
    }
}
