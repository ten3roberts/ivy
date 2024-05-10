use flax::World;
use ivy_assets::AssetCache;
use ivy_base::{app::TickEvent, layer::events::EventRegisterContext, App, Layer};
use ivy_wgpu::{driver::WinitDriver, events::KeyboardInput, layer::GraphicsLayer};
use tracing_subscriber::{layer::SubscriberExt, registry, util::SubscriberInitExt, EnvFilter};
use tracing_tree::HierarchicalLayer;

pub fn main() -> anyhow::Result<()> {
    registry()
        .with(EnvFilter::from_default_env())
        .with(HierarchicalLayer::default().with_indent_lines(true))
        .init();

    if let Err(err) = App::builder()
        .with_driver(WinitDriver::new())
        .with_layer(GraphicsLayer::new())
        .with_layer(LogicLayer::new())
        .run()
    {
        tracing::error!("{err:?}");
        Err(err)
    } else {
        Ok(())
    }
}

pub struct LogicLayer {}

impl Default for LogicLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl LogicLayer {
    pub fn new() -> Self {
        Self {}
    }
}

impl Layer for LogicLayer {
    fn register(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()> {
        events.subscribe(|_, _, _, event: &KeyboardInput| {
            tracing::info!(?event);
            Ok(())
        });

        Ok(())
    }
}
