use flax::error::MissingComponent;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Physics ECS error")]
    EcsError(#[from] flax::Error),
}

impl From<MissingComponent> for Error {
    fn from(v: MissingComponent) -> Self {
        Error::EcsError(v.into())
    }
}
