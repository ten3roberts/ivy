use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Vulkan(#[from] ivy_vulkan::Error),

    #[error("Gltf import failed{}: {0}", .1.as_ref().map(|path| format!(" for {:?}", path)).unwrap_or_default())]
    GltfImport(gltf::Error, Option<PathBuf>),

    #[error("Gltf sparse accessors are not supported")]
    SparseAccessor,

    #[error("Failed to create window")]
    WindowCreation,

    #[error(transparent)]
    ResourceError(#[from] ivy_resources::Error),

    #[error(transparent)]
    ComponentError(#[from] hecs::ComponentError),

    #[error(transparent)]
    NoSuchEntity(#[from] hecs::NoSuchEntity),

    #[error("The maximum number of cameras have been reached ({0})")]
    CameraLimit(u32),
}
