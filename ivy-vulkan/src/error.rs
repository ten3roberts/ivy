use std::{ffi::CString, path::PathBuf};

use ash::vk;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to load vulkan library")]
    LibLoading,
    #[error("Vulkan Error {0}")]
    Vulkan(#[from] vk::Result),
    #[error("Failed to allocate device memory. {0}")]
    MemoryAllocation(#[from] vk_mem::Error),
    #[error("GLFW is not capable of creating vulkan surfaces")]
    SurfaceSupport,
    #[error("Failed to create a vulkan instance. {0}")]
    InstanceCreation(#[from] ash::InstanceError),
    #[error("Missing required extensions: {0:?}")]
    MissingExtensions(Vec<CString>),
    #[error("Missing required instance layers: {0:?}")]
    MissingLayers(Vec<CString>),
    #[error("No suitable physical device was found")]
    UnsuitableDevice,
    #[error("IO error {0}")]
    IO(#[from] std::io::Error),

    #[error(
        "Insufficient buffer size. Trying to write {size} bytes to buffer of {max_size} bytes"
    )]
    BufferOverflow {
        size: vk::DeviceSize,
        max_size: vk::DeviceSize,
    },
    #[error("Failed to load image file {0}")]
    ImageLoading(PathBuf),

    #[error("Unsupported layout transition from {0:?} to {1:?}")]
    UnsupportedLayoutTransition(vk::ImageLayout, vk::ImageLayout),

    #[error("SPIR-V reflection error: {0}")]
    SpirvReflection(&'static str),
}
