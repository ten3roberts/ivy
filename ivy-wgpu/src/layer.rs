use std::sync::Arc;

use flax::World;
use ivy_assets::AssetCache;
use ivy_base::Layer;
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    events::{ApplicationReady, RedrawEvent, ResizedEvent},
    renderer::Renderer,
    Gpu,
};

/// Graphics layer
///
/// Manages window and rendering
pub struct GraphicsLayer {
    renderer: Option<Renderer>,
}

impl GraphicsLayer {
    /// Create a new graphics layer
    pub fn new() -> Self {
        Self { renderer: None }
    }

    fn on_application_ready(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        window: Arc<Window>,
    ) -> Result<(), anyhow::Error> {
        tracing::info!("creating gpu instance with surface");
        let (gpu, surface) = futures::executor::block_on(Gpu::with_surface(window));

        assets.register_service(gpu.clone());

        self.renderer = Some(Renderer::new(gpu, surface));

        Ok(())
    }

    fn on_draw(&mut self, assets: &AssetCache, world: &mut World) -> Result<(), anyhow::Error> {
        if let Some(renderer) = &mut self.renderer {
            renderer.update(world, assets);
            renderer.draw(assets)?;
        }

        Ok(())
    }

    fn on_resize(&mut self, _: &mut World, physical_size: PhysicalSize<u32>) -> anyhow::Result<()> {
        if let Some(renderer) = &mut self.renderer {
            renderer.resize(physical_size);
        } else {
            tracing::warn!("renderer not initialized");
        }

        Ok(())
    }
}

impl Default for GraphicsLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Layer for GraphicsLayer {
    fn register(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        mut events: ivy_base::layer::events::EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        events.subscribe(
            |this, world, assets, ApplicationReady(window): &ApplicationReady| {
                this.on_application_ready(world, assets, window.clone())
            },
        );

        events.subscribe(|this, world, assets, RedrawEvent| this.on_draw(assets, world));
        events.subscribe(|this, world, _, ResizedEvent { physical_size }| {
            this.on_resize(world, *physical_size)
        });

        Ok(())
    }
}
