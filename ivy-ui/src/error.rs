use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error(transparent)]
    Graphics(#[from] ivy_graphics::Error),

    #[error(transparent)]
    ResourceError(#[from] ivy_resources::Error),

    #[error(transparent)]
    ComponentError(#[from] hecs::ComponentError),

    #[error(transparent)]
    NoSuchEntity(#[from] hecs::NoSuchEntity),
}

impl From<ivy_vulkan::Error> for Error {
    fn from(e: ivy_vulkan::Error) -> Self {
        Self::Graphics(e.into())
    }
}
