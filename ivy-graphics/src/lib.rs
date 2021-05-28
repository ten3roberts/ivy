//! Provides higher level graphics abstractions like meshes, materials, and more. Builds on top of
//! ivy-vulkan.

mod document;
mod error;
mod material;
mod mesh;
mod shaderpass;
pub mod window;

pub use document::Document;
pub use error::Error;
pub use material::Material;
pub use mesh::{Mesh, Vertex};
pub use shaderpass::ShaderPass;
