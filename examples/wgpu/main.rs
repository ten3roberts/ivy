mod buffer;
mod pipeline;
mod vertex;

use std::sync::Arc;

use color_eyre::{eyre::ContextCompat, Result};
use glam::vec3;
use parking_lot::{RwLock, RwLockReadGuard};
use tracing::*;
use tracing_subscriber::{prelude::*, Registry};
use tracing_tree::HierarchicalLayer;
use wgpu::*;
use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Window, WindowBuilder},
};

use crate::{pipeline::Pipeline, vertex::Vertex};

#[derive(Debug)]
pub struct Gpu {
    surface: Surface,
    device: Device,
    queue: Queue,
    config: RwLock<SurfaceConfiguration>,
}

impl Gpu {
    pub async fn new(window: &Window) -> Result<Arc<Self>> {
        let window_size = window.inner_size();
        let instance = Instance::new(wgpu::Backends::all());
        let surface = unsafe { instance.create_surface(window) };
        let adapter = instance
            .request_adapter(&RequestAdapterOptions {
                power_preference: PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
            })
            .await
            .wrap_err("Failed to find wgpu adapter")?;

        let (device, queue) = adapter
            .request_device(
                &DeviceDescriptor {
                    features: Features::empty(),
                    limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                },
                None,
            )
            .await?;

        let config = SurfaceConfiguration {
            usage: TextureUsages::RENDER_ATTACHMENT,
            format: surface
                .get_preferred_format(&adapter)
                .unwrap_or(TextureFormat::Rgba8UnormSrgb),
            width: window_size.width.max(64),
            height: window_size.height.max(64),
            present_mode: PresentMode::Mailbox,
        };

        surface.configure(&device, &config);

        Ok(Arc::new(Self {
            surface,
            device,
            queue,
            config: RwLock::new(config),
        }))
    }

    pub fn on_resize(&self, size: PhysicalSize<u32>) {
        if size.width > 0 && size.height > 0 {
            info!("Resizing: {size:?}");
            let mut config = self.config.write();
            config.width = size.width;
            config.height = size.height;
            self.surface.configure(&self.device, &config);
        }
    }

    pub fn config(&self) -> RwLockReadGuard<SurfaceConfiguration> {
        self.config.read()
    }

    pub fn on_draw(
        &self,
        pipeline: &Pipeline,
        vb: &buffer::Buffer<Vertex>,
        ib: &buffer::Buffer<u32>,
    ) -> std::result::Result<(), SurfaceError> {
        let target = self.surface.get_current_texture()?;
        let view = target
            .texture
            .create_view(&TextureViewDescriptor::default());

        let mut encoder = self
            .device
            .create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Main Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: true,
                    },
                }],
                depth_stencil_attachment: None,
            });

            render_pass.set_vertex_buffer(0, vb.slice(..));
            render_pass.set_index_buffer(ib.slice(..), IndexFormat::Uint32);
            render_pass.set_pipeline(pipeline.pipeline()); // 2.
            render_pass.draw_indexed(0..ib.len(), 0, 0..1); // 3.
        }

        self.queue.submit([encoder.finish()]);
        target.present();

        Ok(())
    }

    /// Get a reference to the gpu's device.
    #[must_use]
    fn device(&self) -> &Device {
        &self.device
    }

    /// Get a reference to the gpu's queue.
    #[must_use]
    fn queue(&self) -> &Queue {
        &self.queue
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    color_eyre::install()?;

    let subscriber = Registry::default().with(HierarchicalLayer::new(2));
    tracing::subscriber::set_global_default(subscriber)?;

    let events = EventLoop::new();
    let window = WindowBuilder::new()
        .with_inner_size(PhysicalSize::new(800, 600))
        .with_decorations(true)
        .with_title("Ivy")
        .build(&events)?;

    info!("Opening window");

    let gpu = Gpu::new(&window).await?;

    let pipeline = Pipeline::builder()
        .with_source(&tokio::fs::read_to_string("./examples/wgpu/shaders/default.wgsl").await?)
        .with_vertex_layout(Vertex::layout())
        .build(&gpu, "Pipeline");

    let vb = buffer::Buffer::new(
        gpu.clone(),
        "Vertexbuffer",
        BufferUsages::VERTEX,
        &[
            Vertex {
                pos: vec3(-0.5, -0.5, 0.0),
            },
            Vertex {
                pos: vec3(0.5, -0.5, 0.0),
            },
            Vertex {
                pos: vec3(0.5, 0.5, 0.0),
            },
            Vertex {
                pos: vec3(-0.5, 0.5, 0.0),
            },
        ],
    );

    let ib = buffer::Buffer::new(
        gpu.clone(),
        "Indexbuffer",
        BufferUsages::INDEX,
        &[0, 1, 2, 2, 3, 0],
    );

    events.run(move |event, _, ctl| match event {
        Event::RedrawRequested(window_id) if window_id == window.id() => {
            match gpu.on_draw(&pipeline, &vb, &ib) {
                Ok(_) => {}
                Err(SurfaceError::Lost) => gpu.on_resize(window.inner_size()),
                Err(SurfaceError::OutOfMemory) => *ctl = ControlFlow::Exit,
                Err(e) => tracing::warn!("Faild to draw frame: {e}"),
            }
        }
        Event::WindowEvent {
            ref event,
            window_id,
        } if window_id == window.id() => match event {
            WindowEvent::CloseRequested => {
                *ctl = ControlFlow::Exit;
            }
            WindowEvent::Resized(size) => gpu.on_resize(*size),
            WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
                gpu.on_resize(**new_inner_size)
            }
            _ => {}
        },
        _ => {}
    });
}
