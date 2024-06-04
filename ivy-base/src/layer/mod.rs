#![allow(non_snake_case)]
use crate::app::TickEvent;
use crate::systems::update_transform_system;
use downcast_rs::{impl_downcast, Downcast};
use flax::{Schedule, World};
use ivy_assets::AssetCache;

pub mod events;
mod fixed;
mod layer_stack;

pub use fixed::*;
pub use layer_stack::*;

use self::events::{EventRegisterContext, EventRegistry};

// impl<T, L> LayerDesc for L::Desc
// where
//     L: Layer<Desc = T>,
// {
//     type Layer = L;

//     fn register(self, world: &mut World, assets: &AssetCache) -> anyhow::Result<Self::Layer> {
//         L::register(self, world, assets, EventRegisterContext::default())
//     }
// }

/// A layer represents an ordered abstraction of execution logic. Layers are ordered and run in
/// order.
pub trait Layer: 'static {
    /// Description of the layer to add
    fn register(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized;
}

pub trait LayerDyn: 'static + Downcast {
    fn register_dyn(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        events: &mut EventRegistry,
        index: usize,
    ) -> anyhow::Result<()>;
}

impl_downcast!(LayerDyn);

impl<T: Layer> LayerDyn for T {
    fn register_dyn(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        events: &mut EventRegistry,
        index: usize,
    ) -> anyhow::Result<()> {
        self.register(world, assets, EventRegisterContext::new(events, index))
    }
}

pub struct EngineLayer {
    schedule: Schedule,
}

impl EngineLayer {
    pub fn new() -> Self {
        let schedule = Schedule::builder()
            .with_system(update_transform_system())
            .build();

        Self { schedule }
    }
}

impl Default for EngineLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Layer for EngineLayer {
    fn register(
        &mut self,
        _: &mut World,
        _: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()> {
        events.subscribe(|this, world, _, _: &TickEvent| {
            this.schedule.execute_par(world)?;
            Ok(())
        });

        Ok(())
    }
}
