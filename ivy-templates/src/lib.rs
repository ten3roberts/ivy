//! This crate provides a reusable template system for spawning entities into
//! the world.
mod error;
mod key;
mod templates;
mod traits;

pub use error::*;
pub use key::*;
pub use templates::*;
pub use traits::*;
