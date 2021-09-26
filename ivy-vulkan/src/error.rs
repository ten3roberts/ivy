use std::{ffi::CString, path::PathBuf};

use ash::vk::{self, BufferUsageFlags};
use gpu_allocator::AllocationError;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to load vulkan library")]
    LibLoading,
    #[error("Vulkan API error")]
    Vulkan(#[from] vk::Result),
    #[error("Failed to allocate device memory")]
    MemoryAllocation(#[from] AllocationError),
    #[error("GLFW is not capable of creating vulkan surfaces")]
    SurfaceSupport,
    #[error("Failed to create a vulkan instance")]
    InstanceCreation(#[from] ash::InstanceError),
    #[error("Missing required extensions: {0:?}")]
    MissingExtensions(Vec<CString>),
    #[error("Missing required instance layers: {0:?}")]
    MissingLayers(Vec<CString>),
    #[error("No suitable physical device was found")]
    UnsuitableDevice,
    #[error("Io error {} {0} ", .1.as_ref().map(|path| format!("accessing {:?}.", path)).unwrap_or_default())]
    Io(std::io::Error, Option<PathBuf>),

    #[error(
        "Insufficient buffer size. Trying to write {size} bytes to buffer of {max_size} bytes"
    )]
    BufferOverflow {
        size: vk::DeviceSize,
        max_size: vk::DeviceSize,
    },
    #[error("Failed to load image file")]
    ImageLoading(#[from] ivy_image::Error),

    #[error("Unsupported layout transition from {0:?} to {1:?}")]
    UnsupportedLayoutTransition(vk::ImageLayout, vk::ImageLayout),

    #[error("SPIR-V reflection error: {0}")]
    SpirvReflection(&'static str),

    #[error("Can not access unaquired swapchain image index")]
    NoCurrentSwapchainImage,

    #[error("Unable to determine descriptor type for buffer with usage: {0:?}")]
    DescriptorType(BufferUsageFlags),

    #[error("Vulkan resource error")]
    ResourceError(#[from] ivy_resources::Error),
}
