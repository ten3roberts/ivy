//! Provides higher level graphics abstractions like meshes, materials, and more. Builds on top of
//! ivy-vulkan.

mod camera;
mod camera_manager;
mod document;
mod error;
mod material;
mod mesh;
mod mesh_renderer;
mod shaderpass;

pub mod components;
pub mod systems;
pub mod window;

pub use camera::{Camera, CameraData, ColorAttachment, DepthAttachment};
pub use camera_manager::{CameraIndex, CameraManager};
pub use document::Document;
pub use error::*;
pub use material::Material;
pub use mesh::{Mesh, Vertex};
pub use mesh_renderer::IndirectMeshRenderer;
pub use shaderpass::ShaderPass;
