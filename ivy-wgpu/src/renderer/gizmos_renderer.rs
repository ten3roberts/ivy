use anyhow::Context;
use bytemuck::Zeroable;
use glam::{Mat4, Vec3, Vec4};
use ivy_core::{engine, gizmos, ColorExt};
use ivy_wgpu_types::{
    shader::{ShaderDesc, TargetDesc},
    BindGroupBuilder, BindGroupLayoutBuilder, Gpu, Shader, TypedBuffer,
};
use wgpu::{
    core::device, BindGroup, BindingType, BufferUsages, Operations, RenderPassColorAttachment,
    RenderPassDepthStencilAttachment, RenderPassDescriptor, SamplerBindingType, SamplerDescriptor,
    ShaderStages, TextureSampleType, TextureUsages, TextureViewDimension,
};

use crate::{
    mesh::{Mesh, Vertex, VertexDesc},
    mesh_desc::MeshData,
    rendergraph::{Dependency, Node, NodeExecutionContext, NodeUpdateContext, TextureHandle},
};

use super::{get_camera_data, CameraData};

pub struct GizmosRendererNode {
    mesh: Mesh,
    shader: Option<Shader>,
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

        let mesh = Mesh::new(gpu, mesh.vertices(), mesh.indices());

        let layout = BindGroupLayoutBuilder::new("gizmos")
            .bind_uniform_buffer(ShaderStages::VERTEX)
            .bind_storage_buffer(ShaderStages::VERTEX)
            .bind(
                ShaderStages::FRAGMENT,
                BindingType::Texture {
                    sample_type: TextureSampleType::Depth,
                    view_dimension: TextureViewDimension::D2,
                    multisampled: true,
                },
            )
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
    fn update(&mut self, ctx: NodeUpdateContext) -> anyhow::Result<()> {
        let gizmos = ctx
            .world
            .get(engine(), gizmos())
            .context("Missing gizmos")?;

        if let Some(camera_data) = get_camera_data(ctx.world) {
            self.camera_buffer.write(&ctx.gpu.queue, 0, &[camera_data]);
        }

        self.data.clear();

        for gizmo in gizmos.sections().iter().flat_map(|val| val.1) {
            match gizmo {
                ivy_core::GizmoPrimitive::Sphere {
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
                ivy_core::GizmoPrimitive::Line {
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

        self.buffer.write(&ctx.gpu.queue, 0, &self.data);

        Ok(())
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
            Shader::new(
                ctx.gpu,
                &ShaderDesc {
                    label: "gizmos",
                    module: &ctx
                        .gpu
                        .device
                        .create_shader_module(wgpu::ShaderModuleDescriptor {
                            label: Some("gizmos"),
                            source: wgpu::ShaderSource::Wgsl(
                                include_str!("../../shaders/gizmos.wgsl").into(),
                            ),
                        }),
                    target: &target,
                    vertex_layouts: &[Vertex::layout()],
                    layouts: &[&self.layout],
                    vertex_entry_point: "vs_main",
                    fragment_entry_point: "fs_main",
                    culling: Default::default(),
                },
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
}

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Debug)]
struct Data {
    world: Mat4,
    color: Vec4,
    billboard_axis: Vec3,
    corner_radius: f32,
}