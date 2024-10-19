use std::sync::Arc;

use ivy_assets::service::Service;
use wgpu::{Backends, Features, SurfaceConfiguration, SurfaceError, SurfaceTexture, TextureFormat};
use winit::{dpi::PhysicalSize, window::Window};

fn device_features() -> wgpu::Features {
    Features::TEXTURE_FORMAT_16BIT_NORM | Features::POLYGON_MODE_LINE
}

/// Represents the basic graphics state, such as the device and queue.
#[derive(Debug, Clone)]
pub struct Gpu {
    pub adapter: Arc<wgpu::Adapter>,
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
}

impl Service for Gpu {}

impl Gpu {
    /// Creates a new Gpu instance with a surface.
    pub async fn headless() -> Self {
        #[cfg(not(target_arch = "wasm32"))]
        let backends = Backends::all();

        #[cfg(target_arch = "wasm32")]
        let backends = Backends::GL;

        tracing::info!(?backends);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            dx12_shader_compiler: Default::default(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find an appropriate adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: device_features(),
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    required_limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                    ..Default::default()
                },
                None, // Trace path
            )
            .await
            .unwrap();

        Self {
            adapter: Arc::new(adapter),
            device: Arc::new(device),
            queue: Arc::new(queue),
        }
    }
    /// Creates a new Gpu instance with a surface.
    pub async fn with_surface(window: Arc<Window>) -> (Self, Surface) {
        #[cfg(not(target_arch = "wasm32"))]
        let backends = Backends::all();

        #[cfg(target_arch = "wasm32")]
        let backends = Backends::GL;

        tracing::info!(?backends);

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            dx12_shader_compiler: Default::default(),
            ..Default::default()
        });

        let window_size = window.inner_size();
        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .expect("Failed to find an appropriate adapter");

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    required_features: device_features(),
                    // WebGL doesn't support all of wgpu's features, so if
                    // we're building for the web we'll have to disable some.
                    required_limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    label: None,
                    ..Default::default()
                },
                None, // Trace path
            )
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);

        let surface_format = surface_caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or_else(|| surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            present_mode: wgpu::PresentMode::AutoNoVsync,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            ..surface
                .get_default_config(&adapter, window_size.width, window_size.height)
                .unwrap()
        };

        surface.configure(&device, &config);

        (
            Self {
                adapter: Arc::new(adapter),
                device: Arc::new(device),
                queue: Arc::new(queue),
            },
            Surface {
                surface,
                config,
                size: window_size,
            },
        )
    }
}

pub struct Surface {
    size: PhysicalSize<u32>,
    surface: wgpu::Surface<'static>,
    config: SurfaceConfiguration,
}

impl Surface {
    pub fn get_current_texture(&self) -> Result<SurfaceTexture, SurfaceError> {
        self.surface.get_current_texture()
    }

    pub fn surface_config(&self) -> &SurfaceConfiguration {
        &self.config
    }

    pub fn resize(&mut self, gpu: &Gpu, new_size: PhysicalSize<u32>) {
        tracing::debug_span!("resize", ?new_size);
        if new_size == self.size {
            tracing::info!(size=?new_size, "Duplicate resize message ignored");
            return;
        }

        if new_size.width > 0 && new_size.height > 0 {
            self.config.width = new_size.width;
            self.config.height = new_size.height;

            self.size = new_size;
            tracing::info!("reconfigure surface {:#?}", self.config);
            self.reconfigure(gpu);
        } else {
            self.size = new_size;
        }
    }

    pub fn reconfigure(&mut self, gpu: &Gpu) {
        self.surface.configure(&gpu.device, &self.config);
    }

    pub fn surface_format(&self) -> TextureFormat {
        self.config.format
    }

    pub fn size(&self) -> PhysicalSize<u32> {
        self.size
    }
}
