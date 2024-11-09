#![allow(non_snake_case)]
use crate::components::{async_commandbuffer, engine, gizmos};
use crate::gizmos::Gizmos;
use crate::systems::update_transform_system;
use crate::AsyncCommandBuffer;
use crate::{app::TickEvent, systems::apply_async_commandbuffers};
use downcast_rs::{impl_downcast, Downcast};
use flax::{Entity, Schedule, World};
use ivy_assets::AssetCache;

pub mod events;

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

/// A layer controls it's own event handling and update logic
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
    fn label(&self) -> &str {
        std::any::type_name::<Self>()
    }

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
    cmd: AsyncCommandBuffer,
}

impl EngineLayer {
    pub fn new() -> Self {
        let cmd = AsyncCommandBuffer::new();
        let schedule = Schedule::builder()
            .with_system(apply_async_commandbuffers(cmd.clone()))
            .with_system(update_transform_system())
            .build();

        Self { cmd, schedule }
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
        world: &mut World,
        _: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()> {
        Entity::builder()
            .set(async_commandbuffer(), self.cmd.clone())
            .set(gizmos(), Gizmos::new())
            .append_to(world, engine())?;

        events.subscribe(|this, world, _, _: &TickEvent| {
            this.schedule.execute_par(world)?;
            Ok(())
        });

        Ok(())
    }
}
