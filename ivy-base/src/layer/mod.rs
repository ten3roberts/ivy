#![allow(non_snake_case)]
use crate::impl_for_tuples;
use crate::systems::update_transform_system;
use crate::Events;
use anyhow::Context;
use flax::{Schedule, World};
use ivy_assets::AssetCache;
use std::time::Duration;

mod fixed;
mod layer_stack;

pub use fixed::*;
pub use layer_stack::*;

/// A layer represents an ordered abstraction of execution logic. Layers are ordered and run in
/// order.
pub trait Layer {
    /// Called for each iteration of the application event loop.
    /// The layer can return an error.
    /// frame_time: The duration between this and the last application frame.
    fn on_update(
        &mut self,
        world: &mut World,
        assets: &mut AssetCache,
        events: &mut Events,
        frame_time: Duration,
    ) -> anyhow::Result<()>;
}

macro_rules! tuple_impl {
    ($($name: ident),*) => {
        impl<$($name: Layer),*> Layer for ($($name,)*) {
            // Draws the scene using the pass [`Pass`] and the provided camera.
            // Note: camera must have gpu side data.
            fn on_update(&mut self, world: &mut World, asset_cache: &mut AssetCache, events: &mut Events, frame_time: Duration) -> anyhow::Result<()> {
                let ($($name,)+) = self;

                ($($name.on_update(world, asset_cache, events, frame_time).with_context(|| format!("Failed to execute {:?}", std::any::type_name::<$name>()))?), *);

                Ok(())
            }
        }
    }
}

// Implement renderer on tuple of renderers and tuple of render handles
impl_for_tuples!(tuple_impl);

pub struct EngineLayer {
    schedule: Schedule,
}

impl EngineLayer {
    pub fn new() -> Self {
        let schedule = Schedule::from([update_transform_system()]);
        Self { schedule }
    }
}

impl Layer for EngineLayer {
    fn on_update(
        &mut self,
        world: &mut World,
        assets: &mut AssetCache,
        events: &mut Events,
        frame_time: Duration,
    ) -> anyhow::Result<()> {
        self.schedule.execute_par(world)?;

        Ok(())
    }
}
