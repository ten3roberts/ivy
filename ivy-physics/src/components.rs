use flax::component;
use glam::Vec3;
use ivy_collision::{BvhNode, CollisionTree};

use crate::{systems::CollisionState, Effector};

component! {
    pub physics_state: PhysicsState,
    pub collision_state: CollisionState,
    pub effector: Effector,
    pub gravity_state: GravityState,
    pub collision_tree: CollisionTree<BvhNode>,
}

pub struct PhysicsState {
    pub(crate) dt: f32,
}

pub struct GravityState {
    pub gravity: Vec3,
}
