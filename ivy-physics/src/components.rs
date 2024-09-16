use flax::component;
use glam::Vec3;
use ivy_collision::{BvhNode, CollisionTree};

use crate::{response::Resolver, systems::CollisionState, Effector};

component! {
    pub resolver: Resolver,
    pub collision_state: CollisionState,
    pub effector: Effector,
}
