use thiserror::Error;
#[derive(Debug, Error)]
pub enum Error {
    #[error("Io error")]
    Io(#[from] tokio::io::Error),
    #[error("Failed to load image")]
    Image(image::error::ImageError),
}

pub type Result<T> = std::result::Result<T, Error>;
