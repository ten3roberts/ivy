use flax::{component::ComponentDesc, Entity};
use thiserror::Error;

#[derive(Error, Debug, Clone)]
#[error("Missing input target component {target:?} on entity {entity}")]
pub struct MissingTargetError {
    pub target: ComponentDesc,
    pub entity: Entity,
}
