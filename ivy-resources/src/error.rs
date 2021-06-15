use thiserror::Error;

#[derive(Debug, Error, Clone, Copy)]
pub enum Error {
    #[error("Invalid handle for {0}")]
    InvalidHandle(&'static str),
}
