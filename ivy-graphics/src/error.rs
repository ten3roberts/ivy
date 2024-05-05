use ivy_base::Extent;
use std::{io, path::PathBuf};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Graphics vulkan error")]
    Vulkan(#[from] ivy_vulkan::Error),

    #[error("Failed to initialize glfw")]
    GlfwInitError(#[from] glfw::InitError),

    #[error("Gltf import failed{}: {0}", .1.as_ref().map(|path| format!(" for {:?}", path)).unwrap_or_default())]
    GltfImport(gltf::Error, Option<PathBuf>),

    #[error("Gltf sparse accessors are not supported")]
    SparseAccessor,

    #[error("Failed to create window")]
    WindowCreation,

    #[error("Failed to pack rectangles for texture atlas of size: {0:?}")]
    RectanglePack(Extent),

    #[error("Key does not exist in the atlas")]
    InvalidAtlasKey,

    #[error("The requested document node {0:?} does not exist")]
    UnknownDocumentNode(String),

    #[error("Attempt to create mesh with no vertices")]
    EmptyMesh,

    #[error("No armature was present for skin")]
    MissingArmature,

    #[error("Unable to locate root of armature")]
    MissingRoot,

    #[error("Failed to decode base64")]
    Base64(#[from] base64::DecodeError),

    #[error("Io Error")]
    Io(#[from] io::Error),

    #[error("Gltf scheme is not supported")]
    UnsupportedScheme,

    #[error("Missing action {0:?} for animator")]
    MissingAnimation(String),
    #[error("The animation index was out of bounds.\nAttempt to index animation {0}")]
    InvalidAnimation(usize),

    #[error("ECS error")]
    Flax(#[from] flax::Error),
}
