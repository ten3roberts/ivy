use thiserror::Error;

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
