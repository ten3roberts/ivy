use std::sync::Arc;

use anyhow::Context;
use flax::{component, World};
use ivy_assets::AssetCache;
use ivy_core::{components::engine, Layer};
use ivy_wgpu_types::Surface;
use wgpu::Queue;
use winit::{dpi::PhysicalSize, window::Window};

use crate::{
    events::{ApplicationReady, RedrawEvent, ResizedEvent},
    rendergraph::{ManagedTextureDesc, RenderGraph, TextureHandle},
    Gpu,
};

type OnInitFunc =
    Box<dyn FnOnce(&mut World, &AssetCache, &Gpu, Surface) -> anyhow::Result<Box<dyn Renderer>>>;

type ModifyRenderGraphFunc = Box<
    dyn Send + Sync + FnOnce(&mut World, &mut RenderGraph, &AssetCache, &Gpu) -> anyhow::Result<()>,
>;

/// Control the renderer externally
pub enum RendererCommand {
    ModifyRenderGraph(ModifyRenderGraphFunc),
    UpdateTexture {
        handle: TextureHandle,
        desc: ManagedTextureDesc,
    },
}

impl RendererCommand {
    pub fn modify_rendergraph(
        func: impl 'static
            + Send
            + Sync
            + FnOnce(&mut World, &mut RenderGraph, &AssetCache, &Gpu) -> anyhow::Result<()>,
    ) -> Self {
        Self::ModifyRenderGraph(Box::new(func))
    }
}

component! {
    pub renderer_commands: flume::Sender<RendererCommand>,
}

/// Responsible for rendering the frame
pub trait Renderer {
    fn draw(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        gpu: &Gpu,
        queue: &Queue,
    ) -> anyhow::Result<()>;

    fn process_commands(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        gpu: &Gpu,
        cmds: &mut flume::Receiver<RendererCommand>,
    ) -> anyhow::Result<()>;

    fn on_resize(&mut self, gpu: &Gpu, physical_size: PhysicalSize<u32>);
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

    commands_tx: flume::Sender<RendererCommand>,
    commands_rx: flume::Receiver<RendererCommand>,
}

impl GraphicsLayer {
    /// Create a new graphics layer
    pub fn new<R: 'static + Renderer>(
        mut on_init: impl 'static + FnMut(&mut World, &AssetCache, &Gpu, Surface) -> anyhow::Result<R>,
    ) -> Self {
        let (commands_tx, commands_rx) = flume::unbounded();

        Self {
            rendering_state: None,
            on_init: Some(Box::new(move |world, assets, gpu, surface| {
                Ok(Box::new(on_init(world, assets, gpu, surface)?))
            })),
            commands_tx,
            commands_rx,
        }
    }

    fn on_application_ready(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        window: Arc<Window>,
    ) -> Result<(), anyhow::Error> {
        let (gpu, surface) = futures::executor::block_on(Gpu::with_surface(window));

        assets.register_service(gpu.clone());

        let renderer = (self.on_init.take().unwrap())(world, assets, &gpu, surface)?;

        self.rendering_state = Some(RenderingState { gpu, renderer });

        Ok(())
    }

    fn on_draw(&mut self, assets: &AssetCache, world: &mut World) -> Result<(), anyhow::Error> {
        if let Some(state) = &mut self.rendering_state {
            state
                .renderer
                .process_commands(world, assets, &state.gpu, &mut self.commands_rx)
                .context("Failed to process renderer commands before draw")?;

            state
                .renderer
                .draw(world, assets, &state.gpu, &state.gpu.queue)?;
        }

        Ok(())
    }

    fn on_resize(&mut self, _: &mut World, physical_size: PhysicalSize<u32>) -> anyhow::Result<()> {
        if let Some(state) = &mut self.rendering_state {
            state.renderer.on_resize(&state.gpu, physical_size);
        }

        Ok(())
    }
}

impl Layer for GraphicsLayer {
    fn register(
        &mut self,
        world: &mut World,
        _: &AssetCache,
        mut events: ivy_core::layer::events::EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        world.set(engine(), renderer_commands(), self.commands_tx.clone())?;

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
