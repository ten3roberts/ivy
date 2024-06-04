pub mod mesh_renderer;

use flax::{Query, World};
use glam::Mat4;
use ivy_assets::{stored::Store, Asset, AssetCache};
use ivy_base::{main_camera, world_transform, Bundle};
use wgpu::{
    BindGroup, BufferUsages, Extent3d, Operations, RenderPassColorAttachment, ShaderStages,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages,
};
use winit::dpi::PhysicalSize;

use crate::{
    components::{material, mesh, projection_matrix, shader},
    material::MaterialDesc,
    mesh::MeshDesc,
    types::{
        material::Material, BindGroupBuilder, BindGroupLayoutBuilder, Shader, Surface, TypedBuffer,
    },
    Gpu,
};

use self::mesh_renderer::MeshRenderer;

// TODO: rendergraph with surface publish node
pub struct Renderer {
    gpu: Gpu,
    surface: Surface,
    mesh_renderer: MeshRenderer,
    globals: Globals,
    depth_texture: Option<wgpu::TextureView>,
    store: RendererStore,
}

impl Renderer {
    pub fn new(gpu: Gpu, surface: Surface) -> Self {
        Self {
            mesh_renderer: MeshRenderer::new(&gpu, surface.surface_format()),
            globals: Globals::new(&gpu),
            surface,
            gpu,
            depth_texture: None,
            store: Default::default(),
        }
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        self.surface.resize(&self.gpu, new_size);

        self.depth_texture =
            Some(Self::create_depth_texture(&self.gpu, new_size).create_view(&Default::default()));
    }

    fn create_depth_texture(gpu: &Gpu, size: PhysicalSize<u32>) -> wgpu::Texture {
        gpu.device.create_texture(&TextureDescriptor {
            label: "depth".into(),
            size: Extent3d {
                width: size.width,
                height: size.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Depth24Plus,
            usage: TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
    }

    pub fn update(&mut self, world: &mut World, assets: &AssetCache) {
        tracing::debug!("updating renderer");
        if let Some((world_transform, &projection)) =
            Query::new((world_transform(), projection_matrix()))
                .with(main_camera())
                .borrow(world)
                .first()
        {
            let view = world_transform.inverse();

            tracing::debug!("found camera");

            self.globals
                .buffer
                .write(&self.gpu.queue, 0, &[GlobalData { view, projection }]);
        }

        self.mesh_renderer
            .collect(world, assets, &self.gpu, &mut self.store, &self.globals);
    }

    pub fn draw(&mut self, assets: &AssetCache) -> anyhow::Result<()> {
        let output = self.surface.get_current_texture()?;

        let view = output.texture.create_view(&Default::default());

        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: "main_renderpass".into(),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.0,
                            g: 0.1,
                            b: 0.1,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: self
                        .depth_texture
                        .as_ref()
                        .expect("renderer has no surface size"),
                    depth_ops: Some(Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            self.mesh_renderer.draw(
                assets,
                &self.gpu,
                &self.globals,
                &self.store,
                &mut render_pass,
            );
        }

        self.gpu.queue.submit([encoder.finish()]);

        output.present();

        Ok(())
    }
}

#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct ObjectData {
    transform: Mat4,
}

#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct GlobalData {
    view: Mat4,
    projection: Mat4,
}
pub struct Globals {
    bind_group: BindGroup,
    buffer: TypedBuffer<GlobalData>,
    layout: wgpu::BindGroupLayout,
}

impl Globals {
    fn new(gpu: &Gpu) -> Globals {
        let layout = BindGroupLayoutBuilder::new("Globals")
            .bind_uniform_buffer(ShaderStages::VERTEX)
            .build(gpu);

        let buffer = TypedBuffer::new(
            gpu,
            "Globals buffer",
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            &[Default::default()],
        );

        let bind_group = BindGroupBuilder::new("Globals")
            .bind_buffer(&buffer)
            .build(gpu, &layout);

        Self {
            bind_group,
            buffer,
            layout,
        }
    }
}

pub struct RenderObjectBundle {
    pub mesh: MeshDesc,
    pub material: MaterialDesc,
    pub shader: Asset<crate::shader::ShaderDesc>,
}

impl RenderObjectBundle {
    pub fn new(
        mesh: MeshDesc,
        material: MaterialDesc,
        shader: Asset<crate::shader::ShaderDesc>,
    ) -> Self {
        Self {
            mesh,
            material,
            shader,
        }
    }
}

impl Bundle for RenderObjectBundle {
    fn mount(self, entity: &mut flax::EntityBuilder) {
        entity
            .set(mesh(), self.mesh)
            .set(material(), self.material)
            .set(shader(), self.shader);
    }
}

pub struct RendererStore {
    pub materials: Store<Material>,
    pub shaders: Store<Shader>,
    pub bind_groups: Store<BindGroup>,
}

impl RendererStore {
    pub fn new() -> Self {
        Self {
            materials: Store::new(),
            shaders: Store::new(),
            bind_groups: Store::new(),
        }
    }
}

impl Default for RendererStore {
    fn default() -> Self {
        Self::new()
    }
}
