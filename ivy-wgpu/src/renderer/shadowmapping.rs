use std::mem::size_of;

use crate::{
    components::{cast_shadow, forward_pass, light_data, light_kind, shadow_pass},
    renderer::{
        mesh_renderer::MeshRenderer, CameraRenderer, CameraShaderData, RenderContext,
        RendererStore, UpdateContext,
    },
    rendergraph::{
        BufferHandle, Dependency, Node, NodeExecutionContext, NodeUpdateContext, TextureHandle,
    },
    types::{shader::TargetDesc, BindGroupBuilder, BindGroupLayoutBuilder, TypedBuffer},
    Gpu,
};
use flax::{entity_ids, Query, World};
use glam::{Mat4, Vec3};
use itertools::Itertools;
use ivy_core::{world_transform, WorldExt, DEG_45};
use wgpu::{
    BindGroup, BindGroupLayout, Buffer, BufferDescriptor, BufferUsages, Color, Operations,
    RenderPassColorAttachment, RenderPassDescriptor, ShaderStages, TextureUsages,
    TextureViewDescriptor, TextureViewDimension,
};

use crate::components::light_shadow_data;

pub struct LightShadowData {
    pub index: u32,
    pub view: Mat4,
    pub proj: Mat4,
}

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Debug)]
struct LightShadowCamera {
    viewproj: Mat4,
}

pub struct ShadowMapNode {
    // Texture array
    shadow_maps: TextureHandle,
    layout: BindGroupLayout,
    bind_groups: Option<Vec<BindGroup>>,
    lights: Vec<LightShadowCamera>,
    light_camera_buffer: Buffer,
    renderer: MeshRenderer,
    store: RendererStore,
}

impl ShadowMapNode {
    pub fn new(
        world: &mut World,
        gpu: &Gpu,
        shadow_maps: TextureHandle,
        max_shadows: usize,
    ) -> Self {
        let layout = BindGroupLayoutBuilder::new("LightCameraBuffer")
            .bind_uniform_buffer(ShaderStages::VERTEX)
            .build(gpu);

        let renderer = MeshRenderer::new(world, gpu, shadow_pass());

        let align = gpu.device.limits().min_uniform_buffer_offset_alignment as u64;

        let light_camera_buffer = gpu.device.create_buffer(&BufferDescriptor {
            label: Some("light_camera_buffer"),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            size: (align.max(size_of::<LightShadowCamera>() as u64) * max_shadows as u64),
            mapped_at_creation: false,
        });

        Self {
            shadow_maps,
            layout,
            bind_groups: None,
            lights: vec![],
            light_camera_buffer,
            renderer,
            store: RendererStore::new(),
        }
    }
}

impl Node for ShadowMapNode {
    fn update(&mut self, ctx: NodeUpdateContext) -> anyhow::Result<()> {
        let shadow_maps = ctx.get_texture(self.shadow_maps);

        let focus_point = Vec3::ZERO;

        let mut to_add = Vec::new();

        self.lights.clear();

        for (id, &transform, light_data, &kind) in
            Query::new((entity_ids(), world_transform(), light_data(), light_kind()))
                .with(cast_shadow())
                .borrow(ctx.world)
                .iter()
        {
            let (_, rot, pos) = transform.to_scale_rotation_translation();
            let view;
            let proj;

            if kind.is_directional() {
                let direction = rot * -Vec3::Z;
                let distance = 20.0;
                let origin = focus_point + direction.normalize() * distance;
                view = Mat4::from_scale_rotation_translation(Vec3::ONE, rot, origin).inverse();

                let camera_size = 40.0;
                proj = Mat4::orthographic_lh(
                    -camera_size,
                    camera_size,
                    -camera_size,
                    camera_size,
                    0.1,
                    distance * 2.0,
                );
            } else {
                todo!()
            };

            to_add.push((
                id,
                LightShadowData {
                    index: self.lights.len() as u32,
                    view,
                    proj,
                },
            ));

            self.lights.push(LightShadowCamera {
                viewproj: proj * view,
            });
        }

        ctx.world.append_all(light_shadow_data(), to_add)?;

        let mut update_ctx = UpdateContext {
            world: ctx.world,
            assets: ctx.assets,
            gpu: ctx.gpu,
            store: &mut self.store,
            layout: &self.layout,
            camera: Default::default(),
            target_desc: TargetDesc {
                formats: &[],
                depth_format: Some(shadow_maps.format()),
                sample_count: shadow_maps.sample_count(),
            },
        };

        self.renderer.update(&mut update_ctx)?;

        Ok(())
    }

    fn draw(&mut self, ctx: NodeExecutionContext) -> anyhow::Result<()> {
        let shadow_maps = ctx.get_texture(self.shadow_maps);
        let light_shadow_stride = (ctx.gpu.device.limits().min_uniform_buffer_offset_alignment
            as u64)
            .max(size_of::<LightShadowCamera>() as u64);

        for (i, &light) in self.lights.iter().enumerate() {
            ctx.queue.write_buffer(
                &self.light_camera_buffer,
                i as u64 * light_shadow_stride,
                bytemuck::cast_slice(&[light]),
            );
        }
        ctx.queue.write_buffer(
            &self.light_camera_buffer,
            0,
            bytemuck::cast_slice(&self.lights),
        );

        let bind_groups = self.bind_groups.get_or_insert_with(|| {
            (0..self.lights.len())
                .map(|i| {
                    let bind_group = BindGroupBuilder::new("LightCamera")
                        .bind_buffer_slice(
                            &self.light_camera_buffer,
                            i as u64 * light_shadow_stride,
                            size_of::<LightShadowCamera>() as u64,
                        )
                        .build(ctx.gpu, &self.layout);

                    bind_group
                })
                .collect_vec()
        });

        for (i, bind_group) in bind_groups.iter().enumerate() {
            let view = shadow_maps.create_view(&TextureViewDescriptor {
                aspect: wgpu::TextureAspect::DepthOnly,
                dimension: Some(TextureViewDimension::D2),
                base_array_layer: i as u32,
                array_layer_count: Some(1),
                ..Default::default()
            });

            let draw_ctx = RenderContext {
                world: ctx.world,
                assets: ctx.assets,
                gpu: ctx.gpu,
                store: &mut self.store,
                environment: None,
                bind_group,
                layout: &self.layout,
                target_desc: TargetDesc {
                    formats: &[],
                    depth_format: Some(shadow_maps.format()),
                    sample_count: shadow_maps.sample_count(),
                },
            };

            let mut render_pass = ctx.encoder.begin_render_pass(&RenderPassDescriptor {
                label: "shadow_map".into(),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &view,
                    depth_ops: Some(Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            self.renderer.draw(&draw_ctx, &mut render_pass)?;
        }

        Ok(())
    }

    fn read_dependencies(&self) -> Vec<Dependency> {
        vec![]
    }

    fn write_dependencies(&self) -> Vec<Dependency> {
        vec![Dependency::texture(
            self.shadow_maps,
            TextureUsages::RENDER_ATTACHMENT,
        )]
    }
}
