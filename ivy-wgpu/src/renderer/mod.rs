pub mod mesh_renderer;

use std::any::type_name;

use flax::Query;
use glam::{Mat4, UVec2};
use ivy_assets::{stored::Store, Asset};
use ivy_base::{main_camera, world_transform, Bundle};
use wgpu::{
    BindGroup, BufferUsages, Extent3d, ImageCopyTexture, Operations, RenderPassColorAttachment,
    RenderPassDescriptor, ShaderStages, SurfaceTexture, TextureDescriptor, TextureDimension,
    TextureFormat, TextureUsages,
};
use winit::dpi::PhysicalSize;

use crate::{
    components::{material, mesh, projection_matrix, shader},
    material::MaterialDesc,
    mesh::MeshDesc,
    rendergraph::{Dependency, Node, NodeExecutionContext, TextureHandle},
    types::{
        material::Material, BindGroupBuilder, BindGroupLayoutBuilder, Shader, Surface, TypedBuffer,
    },
    Gpu,
};

use self::mesh_renderer::MeshRenderer;

pub struct SwapchainSurfaceNode {
    surface: Surface,
    final_color: TextureHandle,
    current_surface_texture: Option<SurfaceTexture>,
    size: UVec2,
}

impl SwapchainSurfaceNode {
    pub fn new(final_color: TextureHandle, surface: Surface, size: UVec2) -> Self {
        Self {
            surface,
            final_color,
            size,
            current_surface_texture: None,
        }
    }
}

impl Node for SwapchainSurfaceNode {
    fn label(&self) -> &str {
        type_name::<Self>()
    }

    fn draw(&mut self, ctx: crate::rendergraph::NodeExecutionContext) -> anyhow::Result<()> {
        let final_color = ctx.resources.get_texture_data(self.final_color);
        let output = self.surface.get_current_texture()?;

        // ctx.encoder.create_renderpass(&RenderPassDescriptor {
        //     label: "surface_pass"
        //     color_attachments: todo!(),
        //     depth_stencil_attachment: todo!(),
        //     timestamp_writes: todo!(),
        //     occlusion_query_set: todo!(),
        // });
        ctx.encoder.copy_texture_to_texture(
            ImageCopyTexture {
                texture: final_color,
                mip_level: 0,
                origin: Default::default(),
                aspect: wgpu::TextureAspect::All,
            },
            ImageCopyTexture {
                texture: &output.texture,
                mip_level: 0,
                origin: Default::default(),
                aspect: wgpu::TextureAspect::All,
            },
            Extent3d {
                width: self.size.x,
                height: self.size.y,
                depth_or_array_layers: 1,
            },
        );

        self.current_surface_texture = Some(output);

        Ok(())
    }

    fn finish(&mut self) -> anyhow::Result<()> {
        if let Some(output) = self.current_surface_texture.take() {
            output.present()
        }

        Ok(())
    }

    fn read_dependencies(&self) -> Vec<Dependency> {
        vec![Dependency::texture(
            self.final_color,
            TextureUsages::COPY_SRC,
        )]
    }

    fn write_dependencies(&self) -> Vec<Dependency> {
        vec![]
    }
}

// TODO: rendergraph with surface publish node
pub struct CameraNode {
    mesh_renderer: MeshRenderer,
    globals: Globals,
    depth_texture: Option<wgpu::TextureView>,
    store: RendererStore,
    output: TextureHandle,
}

impl CameraNode {
    pub fn new(gpu: &Gpu, output: TextureHandle, size: PhysicalSize<u32>) -> Self {
        let depth_texture = Self::create_depth_texture(gpu, size).create_view(&Default::default());
        Self {
            mesh_renderer: MeshRenderer::new(&gpu),
            globals: Globals::new(&gpu),
            depth_texture: Some(depth_texture),
            store: Default::default(),
            output,
        }
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        // self.surface.resize(&self.gpu, new_size);

        // self.depth_texture =
        //     Some(Self::create_depth_texture(&self.gpu, new_size).create_view(&Default::default()));
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

    pub fn update(&mut self, ctx: &mut NodeExecutionContext, format: TextureFormat) {
        tracing::debug!("updating renderer");
        if let Some((world_transform, &projection)) =
            Query::new((world_transform(), projection_matrix()))
                .with(main_camera())
                .borrow(ctx.world)
                .first()
        {
            let view = world_transform.inverse();

            tracing::debug!("found camera");

            self.globals
                .buffer
                .write(&ctx.gpu.queue, 0, &[GlobalData { view, projection }]);
        }

        self.mesh_renderer.update(
            ctx.world,
            ctx.assets,
            &ctx.gpu,
            &mut self.store,
            &self.globals,
            format,
        );
    }
}

impl Node for CameraNode {
    fn label(&self) -> &str {
        type_name::<Self>()
    }

    fn draw(&mut self, mut ctx: crate::rendergraph::NodeExecutionContext) -> anyhow::Result<()> {
        let output = ctx.resources.get_texture_data(self.output);

        self.update(&mut ctx, output.format());

        let output_view = output.create_view(&Default::default());
        let mut render_pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: "main_renderpass".into(),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &output_view,
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
            &ctx.assets,
            &ctx.gpu,
            &self.globals,
            &self.store,
            &mut render_pass,
        );

        Ok(())
    }

    fn read_dependencies(&self) -> Vec<Dependency> {
        vec![]
    }

    fn write_dependencies(&self) -> Vec<Dependency> {
        vec![Dependency::texture(
            self.output,
            TextureUsages::RENDER_ATTACHMENT,
        )]
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
