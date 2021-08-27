use ivy_resources::Handle;
use ivy_vulkan::Texture;
use thiserror::Error;

use crate::{NodeIndex, NodeKind};

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

    #[error("Node read attachment is missing corresponding write attachment for {0:?}{1:?}")]
    MissingWrite(NodeIndex, Handle<Texture>),

    #[error("Resource acquisition error {0}")]
    Resource(#[from] ivy_resources::Error),

    #[error("Invalid node index {0:?}")]
    InvalidNodeIndex(NodeIndex),

    #[error("Specified node {0:?} is not the correct kind. Expected {1:?}, found {2:?}")]
    InvalidNodeKind(NodeIndex, NodeKind, NodeKind),
}
