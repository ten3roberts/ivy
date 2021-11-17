use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("Failed to load image from path: {0:?}")]
    FileLoading(PathBuf),
    #[error("Failed to load image from memory")]
    MemoryLoading,
}
