use std::sync::Arc;

use flax::World;
use ivy_assets::AssetCache;
use ivy_base::Layer;
use ivy_wgpu_types::Surface;
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    events::{ApplicationReady, RedrawEvent, ResizedEvent},
    rendergraph::{self, RenderGraph},
    Gpu,
};

pub type CreateRenderGraphFunc =
    Box<dyn FnMut(&mut World, &Gpu, Surface) -> anyhow::Result<(RenderGraph, OnResizeFunc)>>;

pub type OnResizeFunc = Box<dyn FnMut(&Gpu, &mut RenderGraph, PhysicalSize<u32>)>;

struct RenderingState {
    gpu: Gpu,
    rendergraph: RenderGraph,
    on_resize: OnResizeFunc,
}

/// Graphics layer
///
/// Manages window and rendering
pub struct GraphicsLayer {
    rendering_state: Option<RenderingState>,
    create_rendergraph: CreateRenderGraphFunc,
}

impl GraphicsLayer {
    /// Create a new graphics layer
    pub fn new(
        create_rendergraph: impl 'static
            + FnMut(&mut World, &Gpu, Surface) -> anyhow::Result<(RenderGraph, OnResizeFunc)>,
    ) -> Self {
        Self {
            rendering_state: None,
            create_rendergraph: Box::new(create_rendergraph),
        }
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

        tracing::info!("initializing rendergraph");
        let (rendergraph, on_resize) = (self.create_rendergraph)(world, &gpu, surface)?;
        self.rendering_state = Some(RenderingState {
            gpu,
            rendergraph,
            on_resize,
        });

        Ok(())
    }

    fn on_draw(&mut self, assets: &AssetCache, world: &mut World) -> Result<(), anyhow::Error> {
        if let Some(state) = &mut self.rendering_state {
            state
                .rendergraph
                .execute(&state.gpu, &state.gpu.queue, world, assets)?;
        }

        // if let Some(renderer) = &mut self.renderer {
        //     renderer.update(world, assets);
        //     renderer.draw(assets)?;
        // }

        Ok(())
    }

    fn on_resize(&mut self, _: &mut World, physical_size: PhysicalSize<u32>) -> anyhow::Result<()> {
        if let Some(state) = &mut self.rendering_state {
            (state.on_resize)(&state.gpu, &mut state.rendergraph, physical_size);
        }
        // if let Some(renderer) = &mut self.renderer {
        //     renderer.resize(physical_size);
        // } else {
        //     tracing::warn!("renderer not initialized");
        // }

        Ok(())
    }
}

impl Layer for GraphicsLayer {
    fn register(
        &mut self,
        _: &mut World,
        _: &AssetCache,
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
