use crate::systems;
use anyhow::Context;
use hecs::World;
use ivy_collision::{Collision, CollisionTree, Object};
use ivy_core::{Events, Layer};
use ivy_resources::Resources;
use ultraviolet::Vec3;

pub struct PhysicsLayer<const NODE_SIZE: usize> {
    rx: flume::Receiver<Collision>,
}

impl<const NODE_SIZE: usize> PhysicsLayer<NODE_SIZE> {
    pub fn new(
        _world: &mut World,
        resources: &mut Resources,
        events: &mut Events,
        bounds: Vec3,
    ) -> anyhow::Result<Self> {
        let (tx, rx) = flume::unbounded();
        events.subscribe(tx);

        resources
            .default_entry::<CollisionTree<NODE_SIZE>>()?
            .or_insert_with(|| CollisionTree::new(Vec3::zero(), bounds));

        Ok(Self { rx })
    }
}

impl<const NODE_SIZE: usize> Layer for PhysicsLayer<NODE_SIZE> {
    fn on_update(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
        events: &mut Events,
        frame_time: std::time::Duration,
    ) -> anyhow::Result<()> {
        let dt = frame_time.as_secs_f32();
        systems::integrate_angular_velocity(world, dt);
        systems::integrate_velocity(world, dt);
        // physics::systems::gravity_system(world, dt);
        systems::satisfy_objects(world);

        let mut tree = resources
            .get_default_mut::<CollisionTree<NODE_SIZE>>()
            .context("Failed to get default collision tree")?;

        tree.update(world)?;

        let mut gizmos = resources.get_default_mut()?;

        tree.draw_gizmos(world, &mut *gizmos);

        tree.check_collisions::<[&Object; 128]>(world, events)?;

        systems::resolve_collisions(world, self.rx.try_iter())?;

        systems::apply_effectors(world, dt);

        Ok(())
    }
    // add code here
}
