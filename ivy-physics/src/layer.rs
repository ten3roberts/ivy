use std::marker::PhantomData;

use crate::{
    components::{
        collision_state, collision_tree, gravity_state, physics_state, GravityState, PhysicsState,
    },
    connections,
    systems::{self, resolve_collisions_system, CollisionState},
};
use anyhow::Context;
use flax::{events::Event, Entity, Schedule, World};
use glam::Vec3;
use ivy_base::{engine_state, Color, ColorExt, DrawGizmos, Events, Layer};
use ivy_collision::{Collision, CollisionTree, CollisionTreeNode};
use ivy_resources::{DefaultResourceMut, Resources, Storage};

#[derive(Default, Debug, Clone)]
pub struct PhysicsLayerInfo<N> {
    pub gravity: Vec3,
    pub tree_root: N,
    pub debug: bool,
}

impl<N> PhysicsLayerInfo<N> {
    pub fn new(gravity: Vec3, tree_root: N, debug: bool) -> Self {
        Self {
            gravity,
            tree_root,
            debug,
        }
    }
}

pub struct PhysicsLayer<N> {
    gravity: Vec3,
    debug: bool,
    schedule: Schedule,
    marker: PhantomData<N>,
}

impl<N: CollisionTreeNode + Storage> PhysicsLayer<N> {
    pub fn new(
        world: &mut World,
        resources: &mut Resources,
        events: &mut Events,
        info: PhysicsLayerInfo<N>,
    ) -> anyhow::Result<Self> {
        // let rx = events.subscribe();

        let tree_root = info.tree_root;
        Entity::builder()
            .set(physics_state(), PhysicsState { dt: 0.02 })
            .set(
                gravity_state(),
                GravityState {
                    gravity: info.gravity,
                },
            )
            .set(collision_state(), CollisionState::new())
            .append_to(world, engine_state())?;

        // resources
        //     .default_entry::<CollisionTree<N>>()?
        //     .or_insert_with(|| CollisionTree::new(tree_root));

        // resources
        //     .default_entry::<CollisionState>()?
        //     .or_insert_with(|| Default::default());

        // let resolve_collisions =
        //     move |w: &World, r: DefaultResourceMut<_>, e: Read<_>, dt: Read<_>| {
        //         systems::resolve_collisions(w, r, rx.try_iter(), e, dt)
        //     };

        let schedule = Schedule::builder()
            .with_system(systems::gravity())
            .with_system(systems::integrate_velocity())
            // .with_system(connections::update_connections)
            // TODO: merge
            .with_system(ivy_collision::register_system(collision_tree()))
            .flush()
            .with_system(ivy_collision::update_system(collision_tree()))
            .flush()
            .with_system(ivy_collision::check_collisions_system(collision_tree()))
            .flush()
            .with_system(resolve_collisions_system(events.subscribe()))
            .with_system(systems::apply_effectors())
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
        let engine_state = world.entity(engine_state())?;

        engine_state
            .get_mut(physics_state())
            .context("Missing physics state")?
            .dt = frame_time.as_secs_f32();

        if self.debug {
            let root = resources.get_default::<CollisionTree<N>>()?;
            let mut gizmos = resources.get_default_mut()?;
            root.draw_gizmos(&mut gizmos, Color::white());
        }

        self.schedule.execute_par_with(world, events)?;
        Ok(())
    }
}
