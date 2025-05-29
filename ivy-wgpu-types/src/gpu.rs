use std::sync::Arc;

use anyhow::Context;
use gltf::json::extensions::texture::Info;
use ivy_assets::service::Service;
use wgpu::{
    Adapter, Backends, Device, Features, Queue, SurfaceConfiguration, SurfaceError, SurfaceTexture,
    TextureFormat,
};
use winit::{dpi::PhysicalSize, window::Window};

fn required_device_features() -> wgpu::Features {
    Features::TEXTURE_FORMAT_16BIT_NORM
        | Features::POLYGON_MODE_LINE
        | wgpu::Features::INDIRECT_FIRST_INSTANCE
}

/// Gpu creation description.
///
/// Allows customizing creationg of the graphics device.
pub struct GpuCreationDesc {
    /// Required device features. Only use if using a customized render pipeline
    required_features: wgpu::Features,
}

impl Default for GpuCreationDesc {
    fn default() -> Self {
        Self {
            required_features: required_device_features(),
        }
    }
}

impl GpuCreationDesc {
    /// Not all backends are supported.
    ///
    /// DirectX does not support all features by default, or is fully compliant, such as non-zero indirect instance offset.
    fn supported_backends() -> wgpu::Backends {
        Backends::VULKAN | Backends::METAL | Backends::BROWSER_WEBGPU | Backends::GL
    }

    pub async fn request_device(&self, adapter: &Adapter) -> anyhow::Result<(Device, Queue)> {
        adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: "device".into(),
                    required_features: self.required_features,
                    required_limits: if cfg!(target_arch = "wasm32") {
                        wgpu::Limits::downlevel_webgl2_defaults()
                    } else {
                        wgpu::Limits::default()
                    },
                    ..Default::default()
                },
                None,
            )
            .await
            .context("Failed to acquire gpu device")
    }
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
    pub async fn headless(desc: GpuCreationDesc) -> anyhow::Result<Self> {
        let backends = GpuCreationDesc::supported_backends();

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
            .context("Failed to find an appropriate adapter")?;

        let (device, queue) = desc.request_device(&adapter).await?;

        Ok(Self {
            adapter: Arc::new(adapter),
            device: Arc::new(device),
            queue: Arc::new(queue),
        })
    }
    /// Creates a new Gpu instance with a surface.
    pub async fn with_surface(
        window: Arc<Window>,
        desc: GpuCreationDesc,
    ) -> anyhow::Result<(Self, Surface)> {
        let backends = GpuCreationDesc::supported_backends();

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
            .context("Failed to find an appropriate adapter")?;

        tracing::info!("created adapter: {instance:?} {adapter:?}");
        let (device, queue) = desc.request_device(&adapter).await?;

        tracing::info!("created device {device:?}");

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

        Ok((
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
        ))
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
        if new_size == self.size {
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
