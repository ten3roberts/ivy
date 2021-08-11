use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error, Clone, Copy)]
pub enum Error {
    #[error("Invalid handle for {0}")]
    InvalidHandle(&'static str),

    #[error("Resource cache for '{0}' cannot be immutably borrowed while it is mutably borrowed")]
    Borrow(&'static str),
    #[error("Resource cache for '{0}' cannot be mutably borrowed while it is immutably borrowed")]
    BorrowMut(&'static str),
}
