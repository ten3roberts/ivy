use std::marker::PhantomData;

use crate::systems;
use anyhow::Context;
use hecs::World;
use ivy_base::{DrawGizmos, Events, Layer};
use ivy_collision::{Collision, CollisionTree, Node, Object};
use ivy_resources::{Resources, Storage};

pub struct PhysicsLayer<N> {
    rx: flume::Receiver<Collision>,
    marker: PhantomData<N>,
}

impl<N: Node + Storage> PhysicsLayer<N> {
    pub fn new(
        _world: &mut World,
        resources: &mut Resources,
        events: &mut Events,
        tree_root: N,
    ) -> anyhow::Result<Self> {
        let (tx, rx) = flume::unbounded();
        events.subscribe(tx);

        resources
            .default_entry::<CollisionTree<N>>()?
            .or_insert_with(|| CollisionTree::new(tree_root));

        Ok(Self {
            rx,
            marker: PhantomData,
        })
    }
}

impl<N: Node + Storage + DrawGizmos> Layer for PhysicsLayer<N> {
    fn on_update(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
        events: &mut Events,
        frame_time: std::time::Duration,
    ) -> anyhow::Result<()> {
        // let _scope = TimedScope::new(|elapsed| eprintln!("Physics layer took {:.3?}", elapsed));
        let dt = frame_time.as_secs_f32();
        systems::integrate_angular_velocity(world, dt);
        systems::integrate_velocity(world, dt);

        let mut tree = resources
            .get_default_mut::<CollisionTree<N>>()
            .context("Failed to get default collision tree")?;

        tree.update(world)?;

        tree.check_collisions::<[&Object; 128]>(world, events)?;

        systems::resolve_collisions(world, self.rx.try_iter())?;

        systems::apply_effectors(world, dt);

        crate::connections::update_connections(world)
            .context("Failed to update physics connections")?;

        Ok(())
    }
    // add code here
}
