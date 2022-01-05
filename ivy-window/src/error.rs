use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum Error {
    #[error("Unable to find monitor for window")]
    MissingMonitor,
    #[error("GLFW failed to create a window")]
    WindowCreation,
    #[error("Glfw encountered an error")]
    Glfw(#[from] glfw::Error),
    #[error("Failed to initialize glfw")]
    Init(#[from] glfw::InitError),
}

pub type Result<T> = std::result::Result<T, Error>;
