use std::path::PathBuf;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Vulkan(#[from] ivy_vulkan::Error),

    #[error("Graphics error in UI")]
    Graphics(#[from] ivy_graphics::Error),

    #[error("Io error {} {0} ", .1.as_ref().map(|path| format!("accessing {:?}.", path)).unwrap_or_default())]
    Io(std::io::Error, Option<PathBuf>),

    #[error("Font parsing error: {0:?}")]
    FontParsing(&'static str),

    #[error(transparent)]
    ResourceError(#[from] ivy_resources::Error),

    #[error("Specified glyph {0:?} does not exists in the rasterized font")]
    MissingGlyph(char),

    #[error(transparent)]
    ComponentError(#[from] hecs::ComponentError),

    #[error(transparent)]
    NoSuchEntity(#[from] hecs::NoSuchEntity),
}
