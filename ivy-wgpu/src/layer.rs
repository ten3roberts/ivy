use std::{
    borrow::BorrowMut,
    sync::Arc,
    time::{Duration, Instant},
};

use flax::{Entity, World};
use glam::{Quat, Vec3};
use ivy_assets::AssetCache;
use ivy_base::{driver::Driver, App, EntityBuilderExt, Events, Layer, TransformBundle};
use wgpu::{hal::GetAccelerationStructureBuildSizesDescriptor, ShaderStages};
use winit::{
    application::ApplicationHandler,
    dpi::PhysicalSize,
    event::WindowEvent,
    event_loop::ActiveEventLoop,
    window::{Window, WindowAttributes, WindowId},
};

use crate::{
    events::{ApplicationReady, RedrawEvent, ResizedEvent},
    graphics::{
        material::Material, shader::ShaderDesc, texture::TextureFromPath, BindGroupLayoutBuilder,
        Mesh, Shader, Vertex, VertexDesc,
    },
    renderer::{RenderObjectBundle, Renderer},
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

        self.setup_objects(&gpu, world, assets, surface.surface_format())?;

        self.renderer = Some(Renderer::new(gpu, surface));

        Ok(())
    }

    fn on_draw(&mut self, world: &mut World) -> Result<(), anyhow::Error> {
        if let Some(renderer) = &mut self.renderer {
            renderer.update(world);
            renderer.draw(world)?;
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

    fn setup_objects(
        &self,
        gpu: &Gpu,
        world: &mut World,
        assets: &AssetCache,
        surface_format: wgpu::TextureFormat,
    ) -> anyhow::Result<()> {
        let shader = assets.insert(Shader::new(
            gpu,
            &ShaderDesc {
                label: "diffuse shader",
                source: include_str!("../../assets/shaders/diffuse.wgsl"),
                format: surface_format,
                vertex_layouts: &[Vertex::layout()],
                layouts: &[
                    &BindGroupLayoutBuilder::new("globals")
                        .bind_uniform_buffer(ShaderStages::VERTEX)
                        .build(gpu),
                    &BindGroupLayoutBuilder::new("renderer")
                        .bind_storage_buffer(ShaderStages::VERTEX)
                        .build(gpu),
                    &BindGroupLayoutBuilder::new("material")
                        .bind_sampler(ShaderStages::FRAGMENT)
                        .bind_texture(ShaderStages::FRAGMENT)
                        .build(gpu),
                ],
            },
        ));

        let mesh = assets.insert(Mesh::quad(gpu));

        let material = assets.insert(Material::new(
            gpu,
            assets.load(&TextureFromPath("assets/textures/statue.jpg".into())),
        ));

        tracing::info!("created entity");
        let entity = Entity::builder()
            .mount(RenderObjectBundle::new(mesh, material, shader))
            .mount(TransformBundle::new(Vec3::Z, Quat::IDENTITY, Vec3::ONE))
            .spawn(world);

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

        events.subscribe(|this, world, _, RedrawEvent| this.on_draw(world));
        events.subscribe(|this, world, _, ResizedEvent { physical_size }| {
            this.on_resize(world, *physical_size)
        });

        Ok(())
    }
}
