pub mod gizmos_renderer;
mod light_manager;
pub mod mesh_renderer;
mod object_manager;
pub mod shadowmapping;
pub mod skinned_mesh_renderer;

use std::any::type_name;

use flax::{fetch::entity_refs, Component, EntityRef, Query, World};
use glam::{Mat4, Vec3};
use itertools::Itertools;
use ivy_assets::{
    stored::{Handle, Store},
    AssetCache,
};
use ivy_core::{
    components::{main_camera, world_transform},
    impl_for_tuples,
    palette::Srgb,
    to_linear_vec3, Bundle,
};
use ivy_wgpu_types::shader::TargetDesc;
pub use light_manager::LightManager;
pub use object_manager::ObjectManager;
use wgpu::{
    AddressMode, BindGroup, BindGroupLayout, BufferUsages, Extent3d, FilterMode, Operations, Queue,
    RenderPass, RenderPassColorAttachment, RenderPassDescriptor, ShaderStages, TextureDescriptor,
    TextureUsages, TextureViewDescriptor, TextureViewDimension,
};

use crate::{
    components::{environment_data, mesh, projection_matrix},
    material_desc::MaterialData,
    mesh_desc::MeshDesc,
    rendergraph::{Dependency, Node, NodeUpdateContext, TextureHandle, UpdateResult},
    types::{BindGroupBuilder, BindGroupLayoutBuilder, RenderShader, TypedBuffer},
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

    fn on_resource_changed(&mut self, _resource: crate::rendergraph::ResourceHandle) {}

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
    pub queue: &'a Queue,
    pub store: &'a mut RendererStore,
    pub object_manager: &'a ObjectManager,
    // pub camera_data: &'a CameraShaderData,
    pub layouts: &'a [&'a BindGroupLayout],
    pub bind_groups: &'a [&'a BindGroup],
    pub target_desc: TargetDesc<'a>,
}

pub struct UpdateContext<'a> {
    pub world: &'a mut World,
    pub assets: &'a AssetCache,
    pub gpu: &'a Gpu,
    pub store: &'a mut RendererStore,
    pub object_manager: &'a ObjectManager,
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

#[derive(Debug, Clone, Copy)]
pub struct SkyboxTextures {
    pub environment_map: TextureHandle,
    pub irradiance_map: TextureHandle,
    pub specular_map: TextureHandle,
    pub integrated_brdf: TextureHandle,
}

impl SkyboxTextures {
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

#[derive(Default, Debug, Clone, Copy)]
pub struct EnvironmentData {
    pub fog_color: Srgb,
    pub fog_density: f32,
    pub fog_blend: f32,
}

impl EnvironmentData {
    pub fn new(fog_color: Srgb, fog_density: f32, fog_blend: f32) -> Self {
        Self {
            fog_color,
            fog_density,
            fog_blend,
        }
    }
}

pub fn get_main_camera_data(world: &World) -> Option<CameraData> {
    Query::new(entity_refs())
        .with(main_camera())
        .borrow(world)
        .first()
        .map(|entity| get_camera_data(&entity))
}

pub fn get_camera_data(camera: &EntityRef) -> CameraData {
    let world_transform = camera.get_copy(world_transform()).unwrap_or_default();
    let projection = camera.get_copy(projection_matrix()).unwrap_or_default();
    let env_data = camera.get_copy(environment_data()).unwrap_or_default();

    let view = world_transform.inverse();

    CameraData {
        viewproj: projection * view,
        view,
        projection,
        camera_pos: world_transform.transform_point3(Vec3::ZERO),
        fog_color: to_linear_vec3(env_data.fog_color),
        fog_density: env_data.fog_density,
        fog_blend: env_data.fog_blend,
    }
}

pub struct CameraNode {
    renderer: Box<dyn CameraRenderer>,
    shader_data: CameraShaderData,
    depth_texture: TextureHandle,
    store: RendererStore,
    output: TextureHandle,
    /// 0: camera data
    /// 1: environment map
    /// 2: irradiance map
    /// 3: specular map
    /// 4: integrated brdf
    pub bind_group: Option<BindGroup>,
    light_manager: LightManager,
    skybox: Option<SkyboxTextures>,
    object_manager: Handle<ObjectManager>,
}

impl CameraNode {
    pub fn new(
        gpu: &Gpu,
        depth_texture: TextureHandle,
        output: TextureHandle,
        renderer: impl 'static + CameraRenderer,
        light_manager: LightManager,
        object_manager: Handle<ObjectManager>,
        skybox: Option<SkyboxTextures>,
    ) -> Self {
        Self {
            light_manager,
            object_manager,
            renderer: Box::new(renderer),
            shader_data: CameraShaderData::new(gpu),
            depth_texture,
            store: Default::default(),
            output,
            skybox,
            bind_group: None,
        }
    }
}

impl Node for CameraNode {
    fn label(&self) -> &str {
        type_name::<Self>()
    }

    fn update(&mut self, ctx: NodeUpdateContext) -> anyhow::Result<UpdateResult> {
        let output = ctx.get_texture(self.output);

        let depth = ctx.get_texture(self.depth_texture);

        if let Some(camera) = Query::new(entity_refs())
            .with(main_camera())
            .borrow(ctx.world)
            .first()
        {
            self.shader_data.data = get_camera_data(&camera);

            self.shader_data
                .buffer
                .write(&ctx.gpu.queue, 0, &[self.shader_data.data]);
        }

        self.light_manager.update(&ctx)?;
        let object_manager = ctx.store.get_mut(&self.object_manager);

        object_manager.update(ctx.world, ctx.gpu)?;

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
            layouts: &[
                &self.shader_data.layout,
                self.light_manager.layout(),
                object_manager.bind_group_layout(),
            ],
            camera: self.shader_data.data,
            object_manager,
        })?;

        Ok(UpdateResult::Success)
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

            let (environment_map, irradiance_map, specular_map, integrated_brdf) =
                match &self.skybox {
                    Some(v) => (
                        ctx.get_texture(v.environment_map)
                            .create_view(&cubemap_view),
                        ctx.get_texture(v.irradiance_map).create_view(&cubemap_view),
                        ctx.get_texture(v.specular_map).create_view(&cubemap_view),
                        ctx.get_texture(v.integrated_brdf)
                            .create_view(&Default::default()),
                    ),
                    None => {
                        let default_texture = ctx.gpu.device.create_texture(&TextureDescriptor {
                            label: Some("default_skybox"),
                            size: Extent3d {
                                width: 64,
                                height: 64,
                                depth_or_array_layers: 6,
                            },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: wgpu::TextureFormat::R16Float,
                            usage: TextureUsages::TEXTURE_BINDING,
                            view_formats: &[],
                        });

                        let default_brdf = ctx.gpu.device.create_texture(&TextureDescriptor {
                            label: Some("default_integrated_brdf"),
                            size: Extent3d {
                                width: 64,
                                height: 64,
                                depth_or_array_layers: 1,
                            },
                            mip_level_count: 1,
                            sample_count: 1,
                            dimension: wgpu::TextureDimension::D2,
                            format: wgpu::TextureFormat::R16Float,
                            usage: TextureUsages::TEXTURE_BINDING,
                            view_formats: &[],
                        });

                        (
                            default_texture.create_view(&cubemap_view),
                            default_texture.create_view(&cubemap_view),
                            default_texture.create_view(&cubemap_view),
                            default_brdf.create_view(&Default::default()),
                        )
                    }
                };

            BindGroupBuilder::new("Globals")
                .bind_buffer(&self.shader_data.buffer)
                .bind_texture(&environment_map)
                .bind_texture(&irradiance_map)
                .bind_texture(&specular_map)
                .bind_texture(&integrated_brdf)
                .bind_sampler(&environment_sampler)
                .build(ctx.gpu, &self.shader_data.layout)
        });

        let output = ctx.get_texture(self.output);
        let output_view = output.create_view(&Default::default());

        let object_manager = ctx.store.get_mut(&self.object_manager);

        let render_context = RenderContext {
            world: ctx.world,
            assets: ctx.assets,
            gpu: ctx.gpu,
            queue: ctx.queue,
            store: &mut self.store,
            // camera_data: &self.shader_data,
            target_desc: TargetDesc {
                formats: &[output.format()],
                depth_format: depth.format().into(),
                sample_count: output.sample_count(),
            },
            bind_groups: &[
                bind_group,
                self.light_manager.bind_group().unwrap(),
                object_manager.bind_group(),
            ],
            layouts: &[
                &self.shader_data.layout,
                self.light_manager.layout(),
                object_manager.bind_group_layout(),
            ],
            object_manager,
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
        [
            Dependency::texture(
                self.light_manager.shadow_maps(),
                TextureUsages::TEXTURE_BINDING,
            ),
            Dependency::buffer(
                self.light_manager.shadow_camera_buffer(),
                BufferUsages::STORAGE,
            ),
        ]
        .into_iter()
        .chain(
            self.skybox
                .as_ref()
                .map(|v| {
                    [
                        Dependency::texture(v.environment_map, TextureUsages::TEXTURE_BINDING),
                        Dependency::texture(v.irradiance_map, TextureUsages::TEXTURE_BINDING),
                        Dependency::texture(v.specular_map, TextureUsages::TEXTURE_BINDING),
                        Dependency::texture(v.integrated_brdf, TextureUsages::TEXTURE_BINDING),
                    ]
                })
                .into_iter()
                .flatten(),
        )
        .collect_vec()
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

#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
#[repr(C)]
pub struct CameraData {
    pub viewproj: Mat4,
    pub view: Mat4,
    pub projection: Mat4,
    pub camera_pos: Vec3,
    pub fog_blend: f32,
    pub fog_color: Vec3,
    pub fog_density: f32,
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

pub struct RenderObjectBundle<'a> {
    pub mesh: MeshDesc,
    pub materials: &'a [(Component<MaterialData>, MaterialData)],
}

impl<'a> RenderObjectBundle<'a> {
    pub fn new(mesh: MeshDesc, materials: &'a [(Component<MaterialData>, MaterialData)]) -> Self {
        Self { mesh, materials }
    }
}

impl Bundle for RenderObjectBundle<'_> {
    fn mount(self, entity: &mut flax::EntityBuilder) {
        entity.set(mesh(), self.mesh);

        for (pass, material) in self.materials {
            entity.set(*pass, material.clone());
        }
    }
}

pub struct RendererStore {
    pub shaders: Store<RenderShader>,
    pub bind_groups: Store<BindGroup>,
}

impl RendererStore {
    pub fn new() -> Self {
        Self {
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
