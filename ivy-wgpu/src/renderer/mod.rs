pub mod mesh_renderer;

use std::{any::type_name, sync::Arc};

use flax::{Query, World};
use glam::{vec3, Mat4, Vec3, Vec4};
use itertools::Itertools;
use ivy_assets::{stored::Store, Asset, AssetCache};
use ivy_core::{main_camera, world_transform, Bundle};
use wgpu::{
    BindGroup, BufferUsages, Operations, RenderPass, RenderPassColorAttachment,
    RenderPassDescriptor, ShaderStages, Texture, TextureFormat, TextureUsages,
    TextureViewDescriptor, TextureViewDimension,
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

pub struct RenderContext<'a> {
    pub world: &'a mut World,
    pub assets: &'a AssetCache,
    pub gpu: &'a Gpu,
    pub store: &'a mut RendererStore,
    pub camera_data: &'a CameraShaderData,
    pub format: TextureFormat,
    pub enviroment: &'a EnvironmentData,
    pub bind_group: &'a BindGroup,
}

pub struct UpdateContext<'a> {
    pub world: &'a mut World,
    pub assets: &'a AssetCache,
    pub gpu: &'a Gpu,
    pub store: &'a mut RendererStore,
    pub camera_data: &'a CameraShaderData,
    pub format: TextureFormat,
    pub enviroment: &'a EnvironmentData,
}

pub trait CameraRenderer {
    fn update(&mut self, ctx: &mut UpdateContext) -> anyhow::Result<()>;

    fn draw<'s>(
        &'s mut self,
        ctx: &'s RenderContext<'s>,
        render_pass: &mut RenderPass<'s>,
    ) -> anyhow::Result<()>;
}

impl<A: CameraRenderer, B: CameraRenderer> CameraRenderer for (A, B) {
    fn update(&mut self, ctx: &mut UpdateContext) -> anyhow::Result<()> {
        self.0.update(ctx)?;
        self.1.update(ctx)
    }

    fn draw<'s>(
        &'s mut self,
        ctx: &'s RenderContext<'s>,
        render_pass: &mut RenderPass<'s>,
    ) -> anyhow::Result<()> {
        self.0.draw(ctx, render_pass)?;
        self.1.draw(ctx, render_pass)
    }
}

pub struct EnvironmentData {
    environment_map: TextureHandle,
    irradiance_map: TextureHandle,
    specular_map: TextureHandle,
    integrated_brdf: TextureHandle,
}

impl EnvironmentData {
    pub fn new(
        environment_map: TextureHandle,
        irradiance_map: TextureHandle,
        specular_map: TextureHandle,
        integrated_brdf: TextureHandle,
    ) -> Self {
        Self {
            environment_map,
            irradiance_map,
            specular_map,
            integrated_brdf,
        }
    }
}

pub struct CameraNode {
    renderer: Box<dyn CameraRenderer>,
    shader_data: CameraShaderData,
    depth_texture: TextureHandle,
    store: RendererStore,
    output: TextureHandle,
    environment: EnvironmentData,
    /// 0: camera data
    /// 1: light buffer
    /// 2: environment_map
    /// 3: irradiance_map
    /// 4: specular map
    /// 5: integrated brdf
    pub bind_group: Option<BindGroup>,
}

impl CameraNode {
    pub fn new(
        gpu: &Gpu,
        depth_texture: TextureHandle,
        output: TextureHandle,
        renderer: impl 'static + CameraRenderer,
        environment: EnvironmentData,
    ) -> Self {
        Self {
            renderer: Box::new(renderer),
            shader_data: CameraShaderData::new(gpu),
            depth_texture,
            store: Default::default(),
            output,
            environment,
            bind_group: None,
        }
    }

    pub fn update(
        &mut self,
        ctx: &mut NodeExecutionContext,
        format: TextureFormat,
    ) -> anyhow::Result<()> {
        tracing::debug!("updating renderer");
        if let Some((world_transform, &projection)) =
            Query::new((world_transform(), projection_matrix()))
                .with(main_camera())
                .borrow(ctx.world)
                .first()
        {
            let view = world_transform.inverse();

            self.shader_data.data = CameraData {
                view,
                projection,
                camera_pos: world_transform.transform_point3(Vec3::ZERO),
                padding: 0.0,
            };

            self.shader_data
                .buffer
                .write(&ctx.gpu.queue, 0, &[self.shader_data.data]);
        }

        let light_data = Query::new((world_transform(), light()))
            .borrow(ctx.world)
            .iter()
            .map(|(pos, light)| LightData {
                position: pos.transform_point3(Vec3::ZERO).extend(0.0),
                color: (vec3(light.color.red, light.color.green, light.color.blue)
                    * light.intensity)
                    .extend(1.0),
            })
            .take(self.shader_data.light_buffer.len())
            .collect_vec();

        self.shader_data
            .light_buffer
            .write(&ctx.gpu.queue, 0, &light_data);

        self.renderer.update(&mut UpdateContext {
            world: ctx.world,
            assets: ctx.assets,
            gpu: ctx.gpu,
            store: &mut self.store,
            camera_data: &self.shader_data,
            format,
            enviroment: &self.environment,
        })?;

        Ok(())
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

        self.update(&mut ctx, output.format())?;

        let bind_group = self.bind_group.get_or_insert_with(|| {
            let cubemap_view = TextureViewDescriptor {
                dimension: Some(TextureViewDimension::Cube),
                array_layer_count: Some(6),
                ..Default::default()
            };

            BindGroupBuilder::new("Globals")
                .bind_buffer(&self.shader_data.buffer)
                .bind_buffer(&self.shader_data.light_buffer)
                .bind_texture(
                    &ctx.get_texture(self.environment.environment_map)
                        .create_view(&cubemap_view),
                )
                .bind_texture(
                    &ctx.get_texture(self.environment.irradiance_map)
                        .create_view(&cubemap_view),
                )
                .bind_texture(
                    &ctx.get_texture(self.environment.specular_map)
                        .create_view(&cubemap_view),
                )
                .bind_texture(
                    &ctx.get_texture(self.environment.integrated_brdf)
                        .create_view(&Default::default()),
                )
                .build(ctx.gpu, &self.shader_data.layout)
        });
        let output_view = output.create_view(&Default::default());

        let render_context = RenderContext {
            world: ctx.world,
            assets: ctx.assets,
            gpu: ctx.gpu,
            store: &mut self.store,
            camera_data: &self.shader_data,
            format: output.format(),
            enviroment: &self.environment,
            bind_group,
        };

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

        self.renderer.draw(&render_context, &mut render_pass)?;

        Ok(())
    }

    fn read_dependencies(&self) -> Vec<Dependency> {
        vec![
            Dependency::texture(
                self.environment.environment_map,
                TextureUsages::TEXTURE_BINDING,
            ),
            Dependency::texture(
                self.environment.irradiance_map,
                TextureUsages::TEXTURE_BINDING,
            ),
            Dependency::texture(
                self.environment.specular_map,
                TextureUsages::TEXTURE_BINDING,
            ),
            Dependency::texture(
                self.environment.integrated_brdf,
                TextureUsages::TEXTURE_BINDING,
            ),
        ]
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
pub struct CameraData {
    pub view: Mat4,
    pub projection: Mat4,
    pub camera_pos: Vec3,
    padding: f32,
}

#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct LightData {
    pub position: Vec4,
    pub color: Vec4,
}

pub struct CameraShaderData {
    pub data: CameraData,
    buffer: TypedBuffer<CameraData>,
    // TODO: lights should be managed by shared class in mesh renderer
    light_buffer: TypedBuffer<LightData>,
    pub layout: wgpu::BindGroupLayout,
}

impl CameraShaderData {
    fn new(gpu: &Gpu) -> CameraShaderData {
        let layout = BindGroupLayoutBuilder::new("Globals")
            .bind_uniform_buffer(ShaderStages::VERTEX | ShaderStages::FRAGMENT)
            .bind_uniform_buffer(ShaderStages::FRAGMENT)
            .bind_texture_cube(ShaderStages::FRAGMENT)
            .bind_texture_cube(ShaderStages::FRAGMENT)
            .bind_texture_cube(ShaderStages::FRAGMENT)
            .bind_texture(ShaderStages::FRAGMENT)
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

        Self {
            buffer,
            layout,
            light_buffer,
            data: Default::default(),
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
