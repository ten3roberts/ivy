use anyhow::Context;
use bytemuck::Zeroable;
use glam::{Mat4, Vec3, Vec4};
use ivy_core::{
    components::{self, engine},
    ColorExt,
};
use ivy_graphics::mesh::MeshData;
use ivy_wgpu_types::{
    shader::{ShaderDesc, TargetDesc},
    BindGroupBuilder, BindGroupLayoutBuilder, Gpu, RenderShader, TypedBuffer,
};
use wgpu::{
    BindingType, BufferUsages, RenderPassColorAttachment, RenderPassDescriptor, SamplerBindingType,
    SamplerDescriptor, ShaderStages, TextureUsages,
};

use super::{get_main_camera_data, CameraData};
use crate::{
    mesh::{Mesh, Vertex, VertexDesc},
    rendergraph::{
        Dependency, Node, NodeExecutionContext, NodeUpdateContext, TextureHandle, UpdateResult,
    },
};

pub struct GizmosRendererNode {
    mesh: Mesh,
    shader: Option<RenderShader>,
    buffer: TypedBuffer<Data>,
    camera_buffer: TypedBuffer<CameraData>,
    data: Vec<Data>,
    layout: wgpu::BindGroupLayout,
    output: TextureHandle,
    depth_buffer: TextureHandle,
    sampler: wgpu::Sampler,
}

impl GizmosRendererNode {
    pub fn new(gpu: &Gpu, output: TextureHandle, depth_buffer: TextureHandle) -> Self {
        let mesh = MeshData::quad();

        let mesh = Mesh::new(gpu, &Vertex::compose_from_mesh(&mesh), mesh.indices());

        let layout = BindGroupLayoutBuilder::new("gizmos")
            .bind_uniform_buffer(ShaderStages::VERTEX)
            .bind_storage_buffer(ShaderStages::VERTEX)
            .bind_texture_unfiltered(ShaderStages::FRAGMENT)
            .bind(
                ShaderStages::FRAGMENT,
                BindingType::Sampler(SamplerBindingType::NonFiltering),
            )
            .build(gpu);

        let buffer = TypedBuffer::new_uninit(
            gpu,
            "gizmos",
            BufferUsages::STORAGE | BufferUsages::COPY_DST,
            4096,
        );

        let camera_buffer = TypedBuffer::new(
            gpu,
            "gizmos_camera",
            BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            &[CameraData::zeroed()],
        );

        let sampler = gpu.device.create_sampler(&SamplerDescriptor {
            label: Some("gizmos_depth_sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            sampler,
            depth_buffer,
            layout,
            mesh,
            shader: None,
            buffer,
            data: Vec::new(),
            camera_buffer,
            output,
        }
    }
}

impl Node for GizmosRendererNode {
    fn update(&mut self, ctx: NodeUpdateContext) -> anyhow::Result<UpdateResult> {
        let gizmos = ctx
            .world
            .get(engine(), components::gizmos())
            .context("Missing gizmos")?;

        if let Some(camera_data) = get_main_camera_data(ctx.world) {
            self.camera_buffer.write(&ctx.gpu.queue, 0, &[camera_data]);
        }

        self.data.clear();

        for section in gizmos.sections() {
            for primitive in section.primitives() {
                match primitive {
                    ivy_core::gizmos::GizmoPrimitive::Sphere {
                        origin,
                        color,
                        radius,
                    } => {
                        self.data.push(Data {
                            world: Mat4::from_translation(*origin)
                                * Mat4::from_scale(Vec3::splat(*radius)),
                            color: color.to_vec4(),
                            billboard_axis: Vec3::ZERO,
                            corner_radius: 1.0,
                        });
                    }
                    ivy_core::gizmos::GizmoPrimitive::Line {
                        origin,
                        color,
                        dir,
                        radius,
                        corner_radius,
                    } => {
                        self.data.push(Data {
                            world: Mat4::from_translation(*origin + *dir * 0.5)
                                * Mat4::from_scale(Vec3::new(*radius, dir.length() * 0.5, *radius)),
                            color: color.to_vec4(),
                            billboard_axis: dir.normalize(),
                            corner_radius: *corner_radius,
                        });
                    }
                }
            }
        }

        self.buffer.write(&ctx.gpu.queue, 0, &self.data);

        Ok(UpdateResult::Success)
    }

    fn draw(&mut self, ctx: NodeExecutionContext) -> anyhow::Result<()> {
        let output = ctx.get_texture(self.output);
        let depth_buffer = ctx.get_texture(self.depth_buffer);
        let depth_view = depth_buffer.create_view(&Default::default());

        let output_view = output.create_view(&Default::default());

        let bind_group = BindGroupBuilder::new("gizmos")
            .bind_buffer(&self.camera_buffer)
            .bind_buffer(&self.buffer)
            .bind_texture(&depth_view)
            .bind_sampler(&self.sampler)
            .build(ctx.gpu, &self.layout);

        let mut render_pass = ctx.encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("gizmos"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });

        let target = TargetDesc {
            formats: &[output.format()],
            depth_format: None,
            sample_count: output.sample_count(),
        };

        let shader = self.shader.get_or_insert_with(|| {
            let shader_module = ctx
                .gpu
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("gizmos"),
                    source: wgpu::ShaderSource::Wgsl(
                        include_str!("../../shaders/gizmos.wgsl").into(),
                    ),
                });

            RenderShader::new(
                ctx.gpu,
                &ShaderDesc::new("gizmos", &shader_module, &target)
                    .with_vertex_layouts(&[Vertex::layout()])
                    .with_bind_group_layouts(&[&self.layout]),
            )
        });

        render_pass.set_pipeline(shader.pipeline());
        render_pass.set_vertex_buffer(0, self.mesh.vertex_buffer().slice(..));
        render_pass.set_index_buffer(
            self.mesh.index_buffer().slice(..),
            wgpu::IndexFormat::Uint32,
        );

        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw_indexed(0..6, 0, 0..self.data.len() as _);

        Ok(())
    }

    fn read_dependencies(&self) -> Vec<crate::rendergraph::Dependency> {
        vec![
            Dependency::texture(self.output, TextureUsages::RENDER_ATTACHMENT),
            Dependency::texture(self.depth_buffer, TextureUsages::TEXTURE_BINDING),
        ]
    }

    fn write_dependencies(&self) -> Vec<crate::rendergraph::Dependency> {
        vec![]
    }

    fn on_resource_changed(&mut self, _resource: crate::rendergraph::ResourceHandle) {}
}

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Debug)]
struct Data {
    world: Mat4,
    color: Vec4,
    billboard_axis: Vec3,
    corner_radius: f32,
}
