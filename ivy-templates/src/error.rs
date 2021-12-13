use thiserror::Error;

use crate::TemplateKey;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error, Clone)]
pub enum Error {
    #[error("Invalid template key {0:?}")]
    InvalidTemplateKey(TemplateKey),
}
