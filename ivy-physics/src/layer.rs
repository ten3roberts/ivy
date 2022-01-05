use std::marker::PhantomData;

use crate::{connections, systems};
use hecs::World;
use hecs_schedule::{Read, Schedule, SubWorld, System};
use ivy_base::{Color, DeltaTime, DrawGizmos, Events, Gravity, Layer};
use ivy_collision::{CollisionTree, CollisionTreeNode};
use ivy_resources::{Resources, Storage};

#[derive(Default, Debug, Clone)]
pub struct PhysicsLayerInfo<N> {
    pub gravity: Gravity,
    pub tree_root: N,
    pub debug: bool,
}

impl<N> PhysicsLayerInfo<N> {
    pub fn new(gravity: Gravity, tree_root: N, debug: bool) -> Self {
        Self {
            gravity,
            tree_root,
            debug,
        }
    }
}

pub struct PhysicsLayer<N> {
    gravity: Gravity,
    debug: bool,
    schedule: Schedule,
    marker: PhantomData<N>,
}

impl<N: CollisionTreeNode + Storage> PhysicsLayer<N> {
    pub fn new(
        _world: &mut World,
        resources: &mut Resources,
        events: &mut Events,
        info: PhysicsLayerInfo<N>,
    ) -> anyhow::Result<Self> {
        let rx = events.subscribe();

        let tree_root = info.tree_root;

        resources
            .default_entry::<CollisionTree<N>>()?
            .or_insert_with(|| CollisionTree::new(tree_root));

        let resolve_collisions =
            move |w: SubWorld<_>, e: Read<_>| systems::resolve_collisions(w, rx.try_iter(), e);

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
            .add_system(resolve_collisions.named("Resolve Collisions"))
            .add_system(systems::gravity)
            .add_system(systems::apply_effectors)
            .build();

        Ok(Self {
            gravity: info.gravity,
            debug: info.debug,
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
        let mut dt: DeltaTime = frame_time.as_secs_f32().into();

        if self.debug {
            let root = resources.get_default::<CollisionTree<N>>()?;
            let gizmos = resources.get_default_mut()?;
            root.draw_gizmos(gizmos, Color::white());
        }

        self.schedule
            .execute((world, resources, events, &mut dt, &mut self.gravity))?;
        Ok(())
    }
}
