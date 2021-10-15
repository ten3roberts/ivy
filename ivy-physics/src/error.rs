use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error, Clone)]
pub enum Error {
    #[error("Graphics resource error")]
    ResourceError(#[from] ivy_resources::Error),

    #[error("Graphics component fetch error")]
    ComponentError(#[from] hecs::ComponentError),

    #[error("UI entity query error")]
    NoSuchEntity(#[from] hecs::NoSuchEntity),
}
