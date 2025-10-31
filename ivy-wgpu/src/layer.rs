use std::sync::Arc;

use anyhow::Context;
use flax::{component, World};
use ivy_assets::{
    stored::{DynamicStore, Handle},
    AssetCache,
};
use ivy_core::{components::engine, Layer};
use ivy_wgpu_types::Surface;
use wgpu::Queue;
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    events::{ApplicationReady, RedrawEvent, WindowResizedEvent},
    rendergraph::{ManagedTextureDesc, RenderGraph, TextureHandle},
    Gpu,
};

type OnInitFunc = Box<
    dyn FnOnce(
        &mut World,
        &AssetCache,
        &mut DynamicStore,
        &Gpu,
        Surface,
    ) -> anyhow::Result<Box<dyn Renderer>>,
>;

component! {
    pub gpu_instance: Gpu,
    pub render_graph_handle: Handle<RenderGraph>,
}

/// Responsible for rendering the frame
pub trait Renderer {
    fn render_graph(&self) -> Handle<RenderGraph>;

    fn draw(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        store: &mut DynamicStore,
        gpu: &Gpu,
        queue: &Queue,
    ) -> anyhow::Result<()>;

    fn on_resize(&mut self, gpu: &Gpu, store: &DynamicStore, physical_size: PhysicalSize<u32>);
}

struct RenderingState {
    gpu: Gpu,
    renderer: Box<dyn Renderer>,
}

/// Graphics layer
///
/// Manages window and rendering
pub struct GraphicsLayer {
    rendering_state: Option<RenderingState>,
    on_init: Option<OnInitFunc>,
}

impl GraphicsLayer {
    /// Create a new graphics layer
    pub fn new<R: 'static + Renderer>(
        mut on_init: impl 'static
            + FnMut(&mut World, &AssetCache, &mut DynamicStore, &Gpu, Surface) -> anyhow::Result<R>,
    ) -> Self {
        Self {
            rendering_state: None,
            on_init: Some(Box::new(move |world, assets, store, gpu, surface| {
                Ok(Box::new(on_init(world, assets, store, gpu, surface)?))
            })),
        }
    }

    fn on_application_ready(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        store: &mut DynamicStore,
        window: Arc<Window>,
    ) -> Result<(), anyhow::Error> {
        let (gpu, surface) =
            futures::executor::block_on(Gpu::with_surface(window, Default::default()))?;

        world.set(engine(), gpu_instance(), gpu.clone())?;

        assets.register_service(gpu.clone());

        let renderer = (self.on_init.take().unwrap())(world, assets, store, &gpu, surface)?;
        world.set(engine(), render_graph_handle(), renderer.render_graph())?;

        self.rendering_state = Some(RenderingState { gpu, renderer });

        Ok(())
    }

    fn on_draw(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        store: &mut DynamicStore,
    ) -> Result<(), anyhow::Error> {
        if let Some(state) = &mut self.rendering_state {
            state
                .renderer
                .draw(world, assets, store, &state.gpu, &state.gpu.queue)?;
        }

        Ok(())
    }

    fn on_resize(
        &mut self,
        _: &mut World,
        store: &mut DynamicStore,
        physical_size: PhysicalSize<u32>,
    ) -> anyhow::Result<()> {
        if let Some(state) = &mut self.rendering_state {
            state.renderer.on_resize(&state.gpu, store, physical_size);
        }

        Ok(())
    }
}

impl Layer for GraphicsLayer {
    fn register(
        &mut self,
        world: &mut World,
        _: &AssetCache,
        _: &mut DynamicStore,
        mut events: ivy_core::layer::events::EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        events.subscribe(|this, ctx, ApplicationReady(window): &ApplicationReady| {
            this.on_application_ready(ctx.world, ctx.assets, ctx.store, window.clone())
        });

        events.subscribe(|this, ctx, RedrawEvent| this.on_draw(ctx.world, ctx.assets, ctx.store));
        events.subscribe(
            |this,
             ctx,
             WindowResizedEvent {
                 physical_size,
                 logical_size: _,
             }| { this.on_resize(ctx.world, ctx.store, *physical_size) },
        );

        Ok(())
    }
}
