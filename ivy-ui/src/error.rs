use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Vulkan error in UI: {0}")]
    Vulkan(#[from] ivy_vulkan::Error),

    #[error("UI graphics error: {0}")]
    Graphics(#[from] ivy_graphics::Error),

    #[error("Io error {} {0} ", .1.as_ref().map(|path| format!("accessing {:?}.", path)).unwrap_or_default())]
    Io(std::io::Error, Option<PathBuf>),

    #[error("Font parsing error: {0:?}")]
    FontParsing(&'static str),

    #[error("UI resource error: {0}")]
    ResourceError(#[from] ivy_resources::Error),

    #[error("Specified glyph {0:?} does not exists in the rasterized font")]
    MissingGlyph(u16),

    #[error("UI component fetch error: {0}")]
    ComponentError(#[from] hecs::ComponentError),

    #[error("UI entity query error: {0}")]
    NoSuchEntity(#[from] hecs::NoSuchEntity),
}
