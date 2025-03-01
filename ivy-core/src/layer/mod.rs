use downcast_rs::{impl_downcast, Downcast};
use flax::{Entity, Schedule, World};
use ivy_assets::AssetCache;

use crate::{
    app::TickEvent,
    components::{asset_cache, async_commandbuffer, engine, gizmos, request_capture_mouse},
    gizmos::Gizmos,
    systems::apply_async_commandbuffers,
    AsyncCommandBuffer,
};

pub mod events;

use self::events::{EventRegisterContext, EventRegistry};

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
        assets: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()> {
        Entity::builder()
            .set(async_commandbuffer(), self.cmd.clone())
            .set(asset_cache(), assets.clone())
            .set(gizmos(), Gizmos::new())
            .set(request_capture_mouse(), false)
            .append_to(world, engine())?;

        events.subscribe(|this, ctx, _: &TickEvent| {
            this.schedule.execute_par(ctx.world)?;
            Ok(())
        });

        Ok(())
    }
}
