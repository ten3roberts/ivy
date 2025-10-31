use anyhow::Context;
use bytemuck::Zeroable;
use flax::{Component, Query};
use glam::{Mat3, Mat4, Vec3, Vec4};
use ivy_core::{
    components::{self, engine, main_camera, world_transform},
    gizmos::GizmoPrimitive,
    srgba_to_vec4,
};
use ivy_wgpu_types::{
    shader::{Culling, ShaderDesc, TargetDesc},
    BindGroupBuilder, BindGroupLayoutBuilder, Gpu, RenderShader, TypedBuffer,
};
use wgpu::{
    BufferAddress, BufferUsages, Face, FrontFace, LoadOp, RenderPassColorAttachment,
    RenderPassDescriptor, ShaderStages, StoreOp, TextureFormat, TextureUsages,
};

use super::{get_main_camera_data, CameraData};
use crate::{
    mesh::{ColoredVertex, Mesh, MeshDescriptor, Vertex, VertexDesc},
    rendergraph::{
        Dependency, Node, NodeExecutionContext, NodeUpdateContext, TextureHandle, UpdateResult,
    },
};

pub struct GizmosRendererNode {
    mesh: Mesh,
    rect_shader: Option<RenderShader>,
    vertex_shader: Option<RenderShader>,
    buffer: TypedBuffer<Data>,
    camera_buffer: TypedBuffer<CameraData>,
    data: Vec<Data>,
    layout: wgpu::BindGroupLayout,
    output: TextureHandle,
    depth_buffer: TextureHandle,
    draw_index_count: u32,
    main_camera_query: Query<(Component<()>, Component<Mat4>)>,
}

impl GizmosRendererNode {
    pub fn new(gpu: &Gpu, output: TextureHandle, depth_buffer: TextureHandle) -> Self {
        let mesh = Mesh::new(
            gpu,
            &[ColoredVertex::default(); 4],
            &[0; 6],
            MeshDescriptor {
                vertex_buffer_usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                index_buffer_usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
            },
        );

        let layout = BindGroupLayoutBuilder::new("gizmos")
            .bind_uniform_buffer(ShaderStages::VERTEX)
            .bind_storage_buffer(ShaderStages::VERTEX)
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

        Self {
            main_camera_query: Query::new((main_camera(), world_transform())),
            depth_buffer,
            layout,
            mesh,
            rect_shader: None,
            vertex_shader: None,
            buffer,
            data: Vec::new(),
            camera_buffer,
            output,
            draw_index_count: 0,
        }
    }
}

fn align_spherical_billboard(world_transform: Mat4, camera_transform: Mat4) -> Mat4 {
    let camera_rotation = Mat3::from_mat4(camera_transform);

    world_transform * Mat4::from_mat3(camera_rotation)
}

fn align_cylindrical_billboard(
    world_transform: Mat4,
    camera_transform: Mat4,
    billboard_axis: Vec3,
) -> Mat4 {
    let center = world_transform.transform_point3(Vec3::ZERO);

    let to_camera = (center - camera_transform.transform_point3(Vec3::ZERO)).normalize();

    let right = billboard_axis.cross(to_camera).normalize();
    let forward = right.cross(billboard_axis).normalize();

    Mat4::from_translation(center)
        * Mat4::from_mat3(Mat3::from_cols(right, billboard_axis, forward))
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

        self.data.push(Data {
            world: Mat4::IDENTITY,
            color: Vec4::ONE,
            corner_radius: 0.0,
            _padding: Default::default(),
        });

        let (mut vertices, mut indices) = ColoredVertex::quad();

        self.draw_index_count = 0;

        let Some((_, &main_camera_transform)) = self.main_camera_query.borrow(ctx.world).first()
        else {
            return Ok(UpdateResult::Success);
        };

        for section in gizmos.sections() {
            self.draw_index_count += section.indices().len() as u32;
            indices.extend(section.indices().iter().map(|i| i + vertices.len() as u32));
            vertices.extend(
                section
                    .mesh()
                    .iter()
                    .map(|v| ColoredVertex::new(v.pos, srgba_to_vec4(v.color))),
            );

            for primitive in section.primitives() {
                match primitive {
                    GizmoPrimitive::Sphere {
                        origin,
                        color,
                        radius,
                    } => {
                        self.data.push(Data {
                            world: align_spherical_billboard(
                                Mat4::from_translation(*origin),
                                main_camera_transform,
                            ) * Mat4::from_scale(Vec3::splat(*radius)),
                            color: srgba_to_vec4(*color),
                            corner_radius: 1.0,
                            _padding: Default::default(),
                        });
                    }
                    GizmoPrimitive::Line {
                        origin,
                        color,
                        dir,
                        radius,
                        corner_radius,
                    } => {
                        self.data.push(Data {
                            world: align_cylindrical_billboard(
                                Mat4::from_translation(*origin + *dir * 0.5),
                                main_camera_transform,
                                dir.normalize(),
                            ) * Mat4::from_scale(Vec3::new(
                                *radius,
                                dir.length() * 0.5,
                                *radius,
                            )),
                            color: srgba_to_vec4(*color),
                            corner_radius: *corner_radius,
                            _padding: Default::default(),
                        });
                    }
                }
            }
        }

        let vertex_buffer_size = (size_of::<Vertex>() * vertices.len()) as BufferAddress;
        if self.mesh.vertex_buffer().size() >= vertex_buffer_size {
            ctx.gpu.queue.write_buffer(
                self.mesh.vertex_buffer(),
                0,
                bytemuck::cast_slice(&vertices),
            );

            ctx.gpu
                .queue
                .write_buffer(self.mesh.index_buffer(), 0, bytemuck::cast_slice(&indices));
        } else {
            self.mesh = Mesh::new(
                ctx.gpu,
                &vertices,
                &indices,
                MeshDescriptor {
                    vertex_buffer_usage: BufferUsages::VERTEX | BufferUsages::COPY_DST,
                    index_buffer_usage: BufferUsages::INDEX | BufferUsages::COPY_DST,
                },
            )
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
            .build(ctx.gpu, &self.layout);

        let target = TargetDesc {
            formats: &[output.format()],
            depth_format: Some(TextureFormat::Depth24Plus),
            sample_count: output.sample_count(),
        };

        let rect_shader = self.rect_shader.get_or_insert_with(|| -> RenderShader {
            let shader_module = ctx
                .gpu
                .device
                .create_shader_module(wgpu::ShaderModuleDescriptor {
                    label: Some("gizmos"),
                    source: wgpu::ShaderSource::Wgsl(
                        include_str!("../../shaders/rect_gizmos.wgsl").into(),
                    ),
                });

            RenderShader::new(
                ctx.gpu,
                &ShaderDesc::new("gizmos", &shader_module, &target)
                    .with_vertex_layouts(&[ColoredVertex::layout()])
                    .with_bind_group_layouts(&[&self.layout]),
            )
        });

        let shader = self.vertex_shader.get_or_insert_with(|| -> RenderShader {
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
                    .with_culling_mode(Culling {
                        cull_mode: Some(Face::Back),
                        front_face: FrontFace::Ccw,
                    })
                    .with_vertex_layouts(&[ColoredVertex::layout()])
                    .with_bind_group_layouts(&[&self.layout]),
            )
        });

        let mut render_pass = ctx.encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("gizmos"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &output_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    store: StoreOp::Store,
                    load: LoadOp::Clear(1.0),
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });

        // Draw primitives
        render_pass.set_pipeline(shader.pipeline());
        render_pass.set_vertex_buffer(0, self.mesh.vertex_buffer().slice(..));
        render_pass.set_index_buffer(
            self.mesh.index_buffer().slice(..),
            wgpu::IndexFormat::Uint32,
        );

        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw_indexed(6..(6 + self.draw_index_count), 0, 0..1);

        render_pass.set_pipeline(rect_shader.pipeline());
        render_pass.set_bind_group(0, &bind_group, &[]);
        render_pass.draw_indexed(0..6, 0, 1..1 + (self.data.len() - 1) as u32);

        Ok(())
    }

    fn read_dependencies(&self) -> Vec<crate::rendergraph::Dependency> {
        vec![Dependency::texture(
            self.output,
            TextureUsages::RENDER_ATTACHMENT,
        )]
    }

    fn write_dependencies(&self) -> Vec<crate::rendergraph::Dependency> {
        vec![Dependency::texture(
            self.depth_buffer,
            TextureUsages::RENDER_ATTACHMENT,
        )]
    }

    fn on_resource_changed(&mut self, _resource: crate::rendergraph::ResourceHandle) {}
}

#[repr(C)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Debug)]
struct Data {
    world: Mat4,
    color: Vec4,
    corner_radius: f32,
    _padding: [f32; 3],
}
