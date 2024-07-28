mod light_manager;
pub mod mesh_renderer;
pub mod shadowmapping;
pub mod skinned_mesh_renderer;

use std::any::type_name;

use flax::{Query, World};
use glam::{Mat4, Vec3};
use ivy_assets::{stored::Store, Asset, AssetCache};
use ivy_core::{impl_for_tuples, main_camera, world_transform, Bundle};
use ivy_wgpu_types::shader::TargetDesc;
use wgpu::{
    AddressMode, BindGroup, BindGroupLayout, BufferUsages, FilterMode, Operations, RenderPass,
    RenderPassColorAttachment, RenderPassDescriptor, ShaderStages, TextureUsages,
    TextureViewDescriptor, TextureViewDimension,
};

use crate::{
    components::{forward_pass, material, mesh, projection_matrix},
    material::PbrMaterial,
    material_desc::MaterialDesc,
    mesh_desc::MeshDesc,
    rendergraph::{Dependency, Node, TextureHandle},
    types::{BindGroupBuilder, BindGroupLayoutBuilder, Shader, TypedBuffer},
    Gpu,
};

pub use light_manager::LightManager;

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

    fn update(&mut self, _ctx: crate::rendergraph::NodeUpdateContext) -> anyhow::Result<()> {
        Ok(())
    }
}

pub struct RenderContext<'a> {
    pub world: &'a mut World,
    pub assets: &'a AssetCache,
    pub gpu: &'a Gpu,
    pub store: &'a mut RendererStore,
    // pub camera_data: &'a CameraShaderData,
    pub environment: Option<&'a EnvironmentData>,
    pub layouts: &'a [&'a BindGroupLayout],
    pub bind_groups: &'a [&'a BindGroup],
    pub target_desc: TargetDesc<'a>,
}

pub struct UpdateContext<'a> {
    pub world: &'a mut World,
    pub assets: &'a AssetCache,
    pub gpu: &'a Gpu,
    pub store: &'a mut RendererStore,
    pub layouts: &'a [&'a BindGroupLayout],
    // pub camera_data: &'a CameraShaderData,
    pub camera: CameraData,
    pub target_desc: TargetDesc<'a>,
}

pub trait CameraRenderer {
    fn update(&mut self, ctx: &mut UpdateContext) -> anyhow::Result<()>;

    fn draw<'s>(
        &'s mut self,
        ctx: &'s RenderContext<'s>,
        render_pass: &mut RenderPass<'s>,
    ) -> anyhow::Result<()>;
}

macro_rules! impl_for_tuples {
    ($($idx: tt => $ty: ident),*) => {
        impl<$($ty: CameraRenderer,)*> CameraRenderer for ($($ty,)*) {
            fn update(&mut self, ctx: &mut UpdateContext) -> anyhow::Result<()> {
                $(self.$idx.update(ctx)?;)*
                Ok(())
            }

            fn draw<'s>(
                &'s mut self,
                ctx: &'s RenderContext<'s>,
                render_pass: &mut RenderPass<'s>,
            ) -> anyhow::Result<()> {
                $(self.$idx.draw(ctx, render_pass)?;)*

                Ok(())
            }
        }
    };
}

impl_for_tuples! { 0 => A }
impl_for_tuples! { 0 => A, 1 => B }
impl_for_tuples! { 0 => A, 1 => B, 2 => C }
impl_for_tuples! { 0 => A, 1 => B, 2 => C, 3 => D }

impl CameraRenderer for Box<dyn CameraRenderer> {
    fn update(&mut self, ctx: &mut UpdateContext) -> anyhow::Result<()> {
        (**self).update(ctx)
    }

    fn draw<'s>(
        &'s mut self,
        ctx: &'s RenderContext<'s>,
        render_pass: &mut RenderPass<'s>,
    ) -> anyhow::Result<()> {
        (**self).draw(ctx, render_pass)
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
    /// 1: environment map
    /// 2: irradiance map
    /// 3: specular map
    /// 4: integrated brdf
    pub bind_group: Option<BindGroup>,
    light_manager: LightManager,
}

impl CameraNode {
    pub fn new(
        gpu: &Gpu,
        depth_texture: TextureHandle,
        output: TextureHandle,
        renderer: impl 'static + CameraRenderer,
        light_manager: LightManager,
        environment: EnvironmentData,
    ) -> Self {
        Self {
            light_manager,
            renderer: Box::new(renderer),
            shader_data: CameraShaderData::new(gpu),
            depth_texture,
            store: Default::default(),
            output,
            environment,
            bind_group: None,
        }
    }
}

impl Node for CameraNode {
    fn label(&self) -> &str {
        type_name::<Self>()
    }

    fn update(&mut self, ctx: crate::rendergraph::NodeUpdateContext) -> anyhow::Result<()> {
        let output = ctx.get_texture(self.output);

        let depth = ctx.get_texture(self.depth_texture);

        tracing::debug!("updating renderer");
        if let Some((world_transform, &projection)) =
            Query::new((world_transform(), projection_matrix()))
                .with(main_camera())
                .borrow(ctx.world)
                .first()
        {
            let view = world_transform.inverse();

            self.shader_data.data = CameraData {
                viewproj: projection * view,
                view,
                projection,
                camera_pos: world_transform.transform_point3(Vec3::ZERO),
                padding: 0.0,
            };

            self.shader_data
                .buffer
                .write(&ctx.gpu.queue, 0, &[self.shader_data.data]);
        }

        self.light_manager.update(&ctx)?;

        self.renderer.update(&mut UpdateContext {
            world: ctx.world,
            assets: ctx.assets,
            gpu: ctx.gpu,
            store: &mut self.store,
            target_desc: TargetDesc {
                formats: &[output.format()],
                depth_format: depth.format().into(),
                sample_count: output.sample_count(),
            },
            layouts: &[&self.shader_data.layout, self.light_manager.layout()],
            camera: self.shader_data.data,
        })?;

        Ok(())
    }

    fn draw(&mut self, ctx: crate::rendergraph::NodeExecutionContext) -> anyhow::Result<()> {
        let depth = ctx.get_texture(self.depth_texture);

        let depth_view = depth.create_view(&Default::default());

        let bind_group = self.bind_group.get_or_insert_with(|| {
            let cubemap_view = TextureViewDescriptor {
                dimension: Some(TextureViewDimension::Cube),
                array_layer_count: Some(6),
                ..Default::default()
            };

            let environment_sampler = ctx.gpu.device.create_sampler(&wgpu::SamplerDescriptor {
                label: Some("environment_sampler"),
                address_mode_u: AddressMode::ClampToEdge,
                address_mode_v: AddressMode::ClampToEdge,
                address_mode_w: AddressMode::ClampToEdge,
                mag_filter: FilterMode::Linear,
                min_filter: FilterMode::Linear,
                mipmap_filter: FilterMode::Linear,
                anisotropy_clamp: 16,
                ..Default::default()
            });

            BindGroupBuilder::new("Globals")
                .bind_buffer(&self.shader_data.buffer)
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
                .bind_sampler(&environment_sampler)
                .build(ctx.gpu, &self.shader_data.layout)
        });

        let output = ctx.get_texture(self.output);
        let output_view = output.create_view(&Default::default());

        let render_context = RenderContext {
            world: ctx.world,
            assets: ctx.assets,
            gpu: ctx.gpu,
            store: &mut self.store,
            // camera_data: &self.shader_data,
            target_desc: TargetDesc {
                formats: &[output.format()],
                depth_format: depth.format().into(),
                sample_count: output.sample_count(),
            },
            environment: Some(&self.environment),
            bind_groups: &[bind_group, self.light_manager.bind_group().unwrap()],
            layouts: &[&self.shader_data.layout, self.light_manager.layout()],
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
                self.light_manager.shadow_maps(),
                TextureUsages::TEXTURE_BINDING,
            ),
            Dependency::buffer(
                self.light_manager.shadow_camera_buffer(),
                BufferUsages::STORAGE,
            ),
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

    fn on_resource_changed(&mut self, _resource: crate::rendergraph::ResourceHandle) {
        self.light_manager.clear();
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
    pub viewproj: Mat4,
    pub view: Mat4,
    pub projection: Mat4,
    pub camera_pos: Vec3,
    padding: f32,
}

pub struct CameraShaderData {
    pub data: CameraData,
    buffer: TypedBuffer<CameraData>,
    // TODO: lights should be managed by shared class in mesh renderer
    pub layout: BindGroupLayout,
}

impl CameraShaderData {
    fn new(gpu: &Gpu) -> CameraShaderData {
        let layout = BindGroupLayoutBuilder::new("Globals")
            .bind_uniform_buffer(ShaderStages::VERTEX | ShaderStages::FRAGMENT)
            .bind_texture_cube(ShaderStages::FRAGMENT)
            .bind_texture_cube(ShaderStages::FRAGMENT)
            .bind_texture_cube(ShaderStages::FRAGMENT)
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_sampler(ShaderStages::FRAGMENT)
            .build(gpu);

        let buffer = TypedBuffer::new(
            gpu,
            "Globals buffer",
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            &[Default::default()],
        );

        Self {
            buffer,
            layout,
            data: Default::default(),
        }
    }
}

pub struct RenderObjectBundle {
    pub mesh: MeshDesc,
    pub material: MaterialDesc,
    pub shader: Asset<crate::shader::ShaderPassDesc>,
}

impl RenderObjectBundle {
    pub fn new(
        mesh: MeshDesc,
        material: MaterialDesc,
        shader: Asset<crate::shader::ShaderPassDesc>,
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
            .set(forward_pass(), self.shader);
    }
}

pub struct RendererStore {
    pub materials: Store<PbrMaterial>,
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
