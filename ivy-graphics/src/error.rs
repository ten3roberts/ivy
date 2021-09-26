use ivy_vulkan::Extent;
use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Graphics vulkan error")]
    Vulkan(#[from] ivy_vulkan::Error),

    #[error("Gltf import failed{}: {0}", .1.as_ref().map(|path| format!(" for {:?}", path)).unwrap_or_default())]
    GltfImport(gltf::Error, Option<PathBuf>),

    #[error("Gltf sparse accessors are not supported")]
    SparseAccessor,

    #[error("Failed to create window")]
    WindowCreation,

    #[error("Graphics resource error")]
    ResourceError(#[from] ivy_resources::Error),

    #[error("Graphics component fetch error")]
    ComponentError(#[from] hecs::ComponentError),

    #[error("UI entity query error")]
    NoSuchEntity(#[from] hecs::NoSuchEntity),

    #[error("Failed to pack rectangles for texture atlas of size: {0:?}")]
    RectanglePack(Extent),

    #[error("Key does not exist in the atlas")]
    InvalidAtlasKey,
}
