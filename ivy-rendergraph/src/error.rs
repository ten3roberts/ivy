use thiserror::Error;

use crate::{NodeIndex, NodeKind, ResourceKind};

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Failed executing rendergraph node")]
    NodeExecution(#[from] anyhow::Error),

    #[error("Rendergraph vulkan error")]
    Vulkan(#[from] ivy_vulkan::Error),

    #[error("Rendergraph graphics error")]
    Graphics(#[from] ivy_graphics::Error),

    #[error("Dependency cycle in rendergraph")]
    DependencyCycle,

    #[error("Node read attachment is missing corresponding write attachment for {2:?} required by node {0:?}: {1:?}")]
    MissingWrite(NodeIndex, &'static str, ResourceKind),

    #[error("Resource acquisition error")]
    Resource(#[from] ivy_resources::Error),

    #[error("Invalid node index {0:?}")]
    InvalidNodeIndex(NodeIndex),

    #[error("Specified node {0:?} is not the correct kind. Expected {1:?}, found {2:?}")]
    InvalidNodeKind(NodeIndex, NodeKind, NodeKind),
}
