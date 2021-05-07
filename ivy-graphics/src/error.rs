use thiserror::Error;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Vulkan(#[from] ivy_vulkan::Error),

    #[error("Gltf import failed: {0}")]
    GltfImport(#[from] gltf::Error),

    #[error("Gltf sparse accessors are not supported")]
    SparseAccessor,

    #[error("Failed to create window")]
    WindowCreation,
}
