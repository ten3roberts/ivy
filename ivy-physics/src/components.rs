use flax::component;
use glam::Vec3;
use ivy_collision::{BvhNode, CollisionTree};

use crate::{systems::CollisionState, Effector};

component! {
    pub(crate) physics_state: PhysicsState,
    pub(crate) collision_state: CollisionState,
    pub(crate) effector: Effector,
    pub(crate) gravity_state: GravityState,
    pub(crate) collision_tree: CollisionTree<BvhNode>,
}

pub struct PhysicsState {
    pub(crate) dt: f32,
}

pub struct GravityState {
    pub gravity: Vec3,
}
