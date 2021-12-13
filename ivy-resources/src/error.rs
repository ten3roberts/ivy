use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error, Clone)]
pub enum Error {
    #[error("Invalid handle for {0:?}")]
    InvalidHandle(&'static str),

    #[error("attempt to use null handle for {0:?}")]
    NullHandle(&'static str),

    #[error("Missing default resource for {0:?}")]
    MissingDefault(&'static str),

    #[error("Resource cache for {0:?} cannot be immutably borrowed while it is mutably borrowed")]
    Borrow(&'static str),
    #[error("Resource cache for {0:?} cannot be mutably borrowed while it is immutably borrowed")]
    BorrowMut(&'static str),
}
