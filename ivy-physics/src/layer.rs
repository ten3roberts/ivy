use crate::{
    components::{
        collision_state, collision_tree, gravity_state, physics_state, GravityState, PhysicsState,
    },
    systems::{self, resolve_collisions_system, CollisionState},
};
use anyhow::Context;
use flax::{Entity, Schedule, World};
use glam::Vec3;
use ivy_assets::AssetCache;
use ivy_base::{engine, gizmos, Color, ColorExt, DrawGizmos, Events, Layer};
use ivy_collision::{BvhNode, CollisionTree, DespawnedSubscriber};

#[derive(Default, Debug, Clone)]
pub struct PhysicsLayerInfo {
    pub gravity: Vec3,
    pub tree_root: BvhNode,
    pub debug: bool,
}

impl PhysicsLayerInfo {
    pub fn new(gravity: Vec3, tree_root: BvhNode, debug: bool) -> Self {
        Self {
            gravity,
            tree_root,
            debug,
        }
    }
}

pub struct PhysicsLayer {
    gravity: Vec3,
    debug: bool,
    schedule: Schedule,
}

impl PhysicsLayer {
    pub fn new(
        world: &mut World,
        assets: &AssetCache,
        events: &mut Events,
        info: PhysicsLayerInfo,
    ) -> anyhow::Result<Self> {
        // let rx = events.subscribe();

        let (despawned_tx, despawned_rx) = flume::unbounded();

        world.subscribe(DespawnedSubscriber::new(despawned_tx));

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
            .set(
                collision_tree(),
                CollisionTree::new(tree_root, despawned_rx),
            )
            .append_to(world, engine())?;

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
        })
    }
}

impl Layer for PhysicsLayer {
    fn on_update(
        &mut self,
        world: &mut World,
        assets: &mut AssetCache,
        events: &mut Events,
        frame_time: std::time::Duration,
    ) -> anyhow::Result<()> {
        let engine_state = world.entity(engine())?;

        engine_state
            .get_mut(physics_state())
            .context("Missing physics state")?
            .dt = frame_time.as_secs_f32();

        if self.debug {
            let root = world.get(engine(), collision_tree())?;
            let gizmos = &mut *world.get_mut(engine(), gizmos())?;

            root.draw_gizmos(gizmos, Color::white());
        }

        self.schedule.execute_par_with(world, events)?;
        Ok(())
    }
}
