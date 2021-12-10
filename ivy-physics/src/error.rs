use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Graphics resource error")]
    ResourceError(#[from] ivy_resources::Error),

    #[error("Physics ECS error")]
    EcsError(#[from] hecs_schedule::Error),
}
