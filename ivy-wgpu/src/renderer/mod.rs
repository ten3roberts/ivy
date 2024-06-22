pub mod mesh_renderer;

use std::any::type_name;

use flax::Query;
use glam::{vec3, Mat4, Vec3, Vec4};
use itertools::Itertools;
use ivy_assets::{stored::Store, Asset};
use ivy_base::{main_camera, world_transform, Bundle};
use wgpu::{
    BindGroup, BufferUsages, Operations, RenderPassColorAttachment, RenderPassDescriptor,
    ShaderStages, TextureFormat, TextureUsages,
};

use crate::{
    components::{light, material, mesh, projection_matrix, shader},
    material::Material,
    material_desc::MaterialDesc,
    mesh_desc::MeshDesc,
    rendergraph::{Dependency, Node, NodeExecutionContext, TextureHandle},
    types::{BindGroupBuilder, BindGroupLayoutBuilder, Shader, TypedBuffer},
    Gpu,
};

use self::mesh_renderer::MeshRenderer;

pub struct MsaaResolve {
    final_color: TextureHandle,
    resolve_target: TextureHandle,
}

impl MsaaResolve {
    pub fn new(final_color: TextureHandle, resolve_target: TextureHandle) -> Self {
        Self {
            final_color,
            resolve_target,
        }
    }
}

impl Node for MsaaResolve {
    fn label(&self) -> &str {
        type_name::<Self>()
    }

    fn draw(&mut self, ctx: crate::rendergraph::NodeExecutionContext) -> anyhow::Result<()> {
        let final_color = ctx
            .get_texture(self.final_color)
            .create_view(&Default::default());

        let resolve_target = ctx
            .get_texture(self.resolve_target)
            .create_view(&Default::default());

        ctx.encoder.begin_render_pass(&RenderPassDescriptor {
            label: "surface_pass".into(),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &final_color,
                resolve_target: Some(&resolve_target),
                ops: Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });

        Ok(())
    }

    fn read_dependencies(&self) -> Vec<Dependency> {
        vec![Dependency::texture(
            self.final_color,
            TextureUsages::COPY_SRC,
        )]
    }

    fn write_dependencies(&self) -> Vec<Dependency> {
        vec![Dependency::texture(
            self.resolve_target,
            TextureUsages::RENDER_ATTACHMENT,
        )]
    }
}

// TODO: rendergraph with surface publish node
pub struct CameraNode {
    mesh_renderer: MeshRenderer,
    globals: Globals,
    depth_texture: TextureHandle,
    store: RendererStore,
    output: TextureHandle,
}

impl CameraNode {
    pub fn new(gpu: &Gpu, depth_texture: TextureHandle, output: TextureHandle) -> Self {
        Self {
            mesh_renderer: MeshRenderer::new(gpu),
            globals: Globals::new(gpu),
            depth_texture,
            store: Default::default(),
            output,
        }
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

            self.globals.buffer.write(
                &ctx.gpu.queue,
                0,
                &[GlobalData {
                    view,
                    projection,
                    camera_pos: world_transform.transform_point3(Vec3::ZERO),
                    padding: 0.0,
                }],
            );
        }

        {
            let light_data = Query::new((world_transform(), light()))
                .borrow(ctx.world)
                .iter()
                .map(|(pos, light)| LightData {
                    position: pos.transform_point3(Vec3::ZERO).extend(0.0),
                    color: (vec3(light.color.red, light.color.green, light.color.blue)
                        * light.intensity)
                        .extend(1.0),
                })
                .take(self.globals.light_buffer.len())
                .collect_vec();

            self.globals
                .light_buffer
                .write(&ctx.gpu.queue, 0, &light_data);
        }

        self.mesh_renderer.update(
            ctx.world,
            ctx.assets,
            ctx.gpu,
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
        let output = ctx.get_texture(self.output);
        let depth_view = ctx
            .get_texture(self.depth_texture)
            .create_view(&Default::default());

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
                view: &depth_view,
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
            ctx.assets,
            ctx.gpu,
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
        vec![
            Dependency::texture(self.output, TextureUsages::RENDER_ATTACHMENT),
            Dependency::texture(self.depth_texture, TextureUsages::RENDER_ATTACHMENT),
        ]
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
    camera_pos: Vec3,
    padding: f32,
}

#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct LightData {
    pub position: Vec4,
    pub color: Vec4,
}

pub struct Globals {
    bind_group: BindGroup,
    buffer: TypedBuffer<GlobalData>,
    light_buffer: TypedBuffer<LightData>,
    layout: wgpu::BindGroupLayout,
}

impl Globals {
    fn new(gpu: &Gpu) -> Globals {
        let layout = BindGroupLayoutBuilder::new("Globals")
            .bind_uniform_buffer(ShaderStages::VERTEX | ShaderStages::FRAGMENT)
            .bind_uniform_buffer(ShaderStages::FRAGMENT)
            .build(gpu);

        let buffer = TypedBuffer::new(
            gpu,
            "Globals buffer",
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            &[Default::default()],
        );

        let light_buffer = TypedBuffer::new(
            gpu,
            "light_buffer",
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            &[Default::default(); 8],
        );

        let bind_group = BindGroupBuilder::new("Globals")
            .bind_buffer(&buffer)
            .bind_buffer(&light_buffer)
            .build(gpu, &layout);

        Self {
            bind_group,
            buffer,
            layout,
            light_buffer,
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
