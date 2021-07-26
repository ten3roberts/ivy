use thiserror::Error;

use crate::NodeIndex;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed executing rendergraph node:\n{0:?}")]
    NodeExecution(#[from] anyhow::Error),

    #[error(transparent)]
    Vulkan(#[from] ivy_vulkan::Error),

    #[error(transparent)]
    Graphics(#[from] ivy_graphics::Error),

    #[error("Dependency cycle in rendergraph")]
    DependencyCycle,

    #[error("Node read attachment is missing corresponding write attachment")]
    MissingWrite,

    #[error("Invalid resource handle {0}")]
    InvalidHandle(#[from] ivy_resources::Error),

    #[error("Invalid node index {0}")]
    InvalidNodeIndex(NodeIndex),
}
