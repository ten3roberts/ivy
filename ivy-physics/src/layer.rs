use std::marker::PhantomData;

use crate::{connections, systems};
use hecs::World;
use hecs_schedule::{Schedule, SubWorld};
use ivy_base::{DeltaTime, DrawGizmos, Events, Layer};
use ivy_collision::{CollisionTree, CollisionTreeNode};
use ivy_resources::{Resources, Storage};

pub struct PhysicsLayer<N> {
    schedule: Schedule,
    marker: PhantomData<N>,
}

impl<N: CollisionTreeNode + Storage> PhysicsLayer<N> {
    pub fn new(
        _world: &mut World,
        resources: &mut Resources,
        events: &mut Events,
        tree_root: N,
    ) -> anyhow::Result<Self> {
        let rx = events.subscribe_flume();

        resources
            .default_entry::<CollisionTree<N>>()?
            .or_insert_with(|| CollisionTree::new(tree_root));

        let schedule = Schedule::builder()
            .add_system(systems::integrate_velocity)
            .add_system(systems::integrate_angular_velocity)
            .add_system(connections::update_connections)
            .add_system(CollisionTree::<N>::register_system)
            .flush()
            .add_system(CollisionTree::<N>::update_system)
            .flush()
            .add_system(CollisionTree::<N>::check_collisions_system)
            .barrier() // Explicit channel dependency
            .add_system(move |w: SubWorld<_>| systems::resolve_collisions(w, rx.try_iter()))
            .add_system(systems::apply_effectors)
            .build();

        eprintln!("Physics layer schedule: {}", schedule.batch_info());

        Ok(Self {
            schedule,
            marker: PhantomData,
        })
    }
}

impl<N: CollisionTreeNode + Storage + DrawGizmos> Layer for PhysicsLayer<N> {
    fn on_update(
        &mut self,
        world: &mut World,
        resources: &mut Resources,
        events: &mut Events,
        frame_time: std::time::Duration,
    ) -> anyhow::Result<()> {
        // let _scope = TimedScope::new(|elapsed| eprintln!("Physics layer took {:.3?}", elapsed));
        let mut dt: DeltaTime = frame_time.as_secs_f32().into();

        self.schedule.execute((world, resources, events, &mut dt))?;
        Ok(())
    }
    // add code here
}
