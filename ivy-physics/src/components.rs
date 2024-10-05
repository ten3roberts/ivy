use flax::component;

use crate::{state::PhysicsState, systems::CollisionState, Effector};

component! {
    pub physics_state: PhysicsState,
    // TODO: remove
    pub collision_state: CollisionState,
    pub effector: Effector,
}
