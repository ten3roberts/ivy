use std::marker::PhantomData;

use crate::systems;
use anyhow::Context;
use hecs::World;
use ivy_collision::{Collision, CollisionTree, Object};
use ivy_core::{Events, Layer, TimedScope};
use ivy_resources::{Resources, Storage};
use smallvec::Array;
use ultraviolet::Vec3;

pub struct PhysicsLayer<T: Array<Item = Object>> {
    rx: flume::Receiver<Collision>,
    marker: PhantomData<T>,
}

impl<T: Array<Item = Object> + Storage> PhysicsLayer<T> {
    pub fn new(
        _world: &mut World,
        resources: &mut Resources,
        events: &mut Events,
        bounds: Vec3,
    ) -> anyhow::Result<Self> {
        let (tx, rx) = flume::unbounded();
        events.subscribe(tx);

        resources
            .default_entry::<CollisionTree<T>>()?
            .or_insert_with(|| CollisionTree::new(Vec3::zero(), bounds));

        Ok(Self {
            rx,
            marker: PhantomData,
        })
    }
}

impl<T: Array<Item = Object> + Storage> Layer for PhysicsLayer<T> {
    fn on_update(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
        events: &mut Events,
        frame_time: std::time::Duration,
    ) -> anyhow::Result<()> {
        let _scope = TimedScope::new(|elapsed| eprintln!("Physics layer took {:.3?}", elapsed));
        let dt = frame_time.as_secs_f32();
        systems::integrate_angular_velocity(world, dt);
        systems::integrate_velocity(world, dt);
        systems::satisfy_objects(world);

        let mut tree = resources
            .get_default_mut::<CollisionTree<T>>()
            .context("Failed to get default collision tree")?;

        tree.update(world)?;

        // eprintln!("{:#?}", *tree);
        // std::thread::sleep(Duration::from_secs(1));
        let mut gizmos = resources.get_default_mut()?;

        tree.draw_gizmos(world, &mut *gizmos);

        tree.check_collisions::<[&Object; 128]>(world, events)?;

        systems::resolve_collisions(world, self.rx.try_iter())?;

        systems::apply_effectors(world, dt);

        Ok(())
    }
    // add code here
}