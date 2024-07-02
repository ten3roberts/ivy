//! Image based lighting for environment lighting

use glam::{Mat4, Vec3};
use image::DynamicImage;
use itertools::Itertools;
use ivy_assets::AssetCache;
use ivy_core::DEG_90;
use ivy_wgpu::{
    types::{shader::ShaderDesc, BindGroupBuilder, BindGroupLayoutBuilder, Shader, TypedBuffer},
    Gpu,
};
use wgpu::{
    vertex_attr_array, BufferUsages, Color, ColorTargetState, CommandEncoder, Extent3d,
    FragmentState, IndexFormat, LoadOp, Operations, PipelineLayoutDescriptor, PrimitiveState,
    RenderPassColorAttachment, RenderPassDescriptor, RenderPipelineDescriptor, Sampler,
    SamplerDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp, Texture,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor,
    TextureViewDimension, VertexBufferLayout, VertexState, VertexStepMode,
};

pub struct EnvironmentMapMode {}

#[repr(C)]
#[derive(Default, bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Debug)]
struct InverseCameraData {
    inv_proj: Mat4,
    inv_view: Mat4,
}

pub struct HdriProcessor {
    viewproj_buffers: [TypedBuffer<Mat4>; 6],
    inv_viewproj: [TypedBuffer<InverseCameraData>; 6],
    sampler: Sampler,
    format: TextureFormat,
}

impl HdriProcessor {
    pub fn new(gpu: &Gpu, format: TextureFormat) -> Self {
        let proj = Mat4::perspective_lh(DEG_90, 1.0, 0.1, 10.0);
        let view_matrices = [
            Mat4::look_at_lh(Vec3::ZERO, Vec3::X, Vec3::Y),
            Mat4::look_at_lh(Vec3::ZERO, -Vec3::X, Vec3::Y),
            Mat4::look_at_lh(Vec3::ZERO, Vec3::Y, -Vec3::Z),
            Mat4::look_at_lh(Vec3::ZERO, -Vec3::Y, Vec3::Z),
            Mat4::look_at_lh(Vec3::ZERO, Vec3::Z, Vec3::Y),
            Mat4::look_at_lh(Vec3::ZERO, -Vec3::Z, Vec3::Y),
        ];

        let viewproj_buffers = view_matrices.map(|v| {
            TypedBuffer::<Mat4>::new(gpu, "hdri_camera_data", BufferUsages::UNIFORM, &[proj * v])
        });

        let inv_viewproj = view_matrices.map(|v| {
            TypedBuffer::new(
                gpu,
                "diffuse_irradiance",
                BufferUsages::UNIFORM,
                &[InverseCameraData {
                    inv_proj: proj.inverse(),
                    inv_view: v.inverse(),
                }],
            )
        });

        let sampler = gpu.device.create_sampler(&SamplerDescriptor {
            label: "material_sampler".into(),
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,

            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        Self {
            viewproj_buffers,
            inv_viewproj,
            sampler,
            format,
        }
    }

    pub fn process_hdri(
        &self,
        gpu: &Gpu,
        encoder: &mut CommandEncoder,
        assets: &AssetCache,
        source: &DynamicImage,
    ) -> Texture {
        let source_hdri = ivy_wgpu::types::texture::texture_from_image(
            gpu,
            assets,
            source,
            ivy_wgpu::types::texture::TextureFromImageDesc {
                label: "hdri".into(),
                format: TextureFormat::Rgba8UnormSrgb,
                mip_level_count: Some(1),
                usage: TextureUsages::TEXTURE_BINDING,
                generate_mipmaps: false,
            },
        )
        .unwrap();

        let bind_group_layout = BindGroupLayoutBuilder::new("hdri")
            .bind_uniform_buffer(ShaderStages::VERTEX)
            .bind_sampler(ShaderStages::FRAGMENT)
            .bind_texture(ShaderStages::FRAGMENT)
            .build(gpu);

        let sampler = gpu.device.create_sampler(&SamplerDescriptor {
            label: "material_sampler".into(),
            min_filter: wgpu::FilterMode::Linear,
            mag_filter: wgpu::FilterMode::Linear,

            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        let bind_groups = self
            .viewproj_buffers
            .iter()
            .map(|v| {
                let bind_group = BindGroupBuilder::new("hdri")
                    .bind_buffer(v.buffer())
                    .bind_sampler(&sampler)
                    .bind_texture(&source_hdri.create_view(&Default::default()))
                    .build(gpu, &bind_group_layout);

                bind_group
            })
            .collect_vec();

        let shader = Shader::new(
            gpu,
            &ShaderDesc {
                label: "hdri_project",
                source: include_str!("../shaders/equirect_project.wgsl"),
                format: self.format,
                vertex_layouts: &[VertexBufferLayout {
                    array_stride: 12,
                    step_mode: VertexStepMode::Vertex,
                    attributes: &vertex_attr_array![0 => Float32x3],
                }],
                layouts: &[&bind_group_layout],
                depth_format: None,
                sample_count: 1,
            },
        );

        let vb = cube_vertices(gpu);
        let ib = cube_indices(gpu);

        const CUBE_MAP_SIZE: Extent3d = Extent3d {
            width: 1024,
            height: 1024,
            depth_or_array_layers: 6,
        };

        let output = gpu.device.create_texture(&TextureDescriptor {
            label: "hdr_cubemap".into(),
            size: CUBE_MAP_SIZE,
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: self.format,
            usage: TextureUsages::RENDER_ATTACHMENT
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        let sides = (0..6)
            .map(|side| {
                output.create_view(&TextureViewDescriptor {
                    base_array_layer: side,
                    array_layer_count: Some(1),
                    dimension: Some(TextureViewDimension::D2),
                    ..Default::default()
                })
            })
            .collect_vec();

        for (side, bind_group) in sides.iter().zip(bind_groups) {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: side,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            tracing::info!("drawing side");

            render_pass.set_pipeline(shader.pipeline());
            render_pass.set_vertex_buffer(0, vb.slice(..));
            render_pass.set_index_buffer(ib.slice(..), IndexFormat::Uint16);
            render_pass.set_bind_group(0, &bind_group, &[]);

            render_pass.draw_indexed(0..36, 0, 0..1);
        }

        output
    }
    pub fn process_diffuse_irradiance(
        &self,
        gpu: &Gpu,
        encoder: &mut CommandEncoder,
        hdri: &Texture,
    ) -> Texture {
        let bind_group_layout = BindGroupLayoutBuilder::new("diffuse_irradiance")
            .bind_uniform_buffer(ShaderStages::FRAGMENT)
            .bind_sampler(ShaderStages::FRAGMENT)
            .bind_texture_cube(ShaderStages::FRAGMENT)
            .build(gpu);

        let bind_groups = self
            .inv_viewproj
            .iter()
            .map(|v| {
                BindGroupBuilder::new("diffuse_irradiance")
                    .bind_buffer(v.buffer())
                    .bind_sampler(&self.sampler)
                    .bind_texture(&hdri.create_view(&TextureViewDescriptor {
                        dimension: Some(TextureViewDimension::Cube),
                        ..Default::default()
                    }))
                    .build(gpu, &bind_group_layout)
            })
            .collect_vec();

        let shader = Shader::new(
            gpu,
            &ShaderDesc {
                label: "diffuse_irradiance",
                source: include_str!("../shaders/diffuse_irradiance.wgsl"),
                format: self.format,
                vertex_layouts: &[],
                layouts: &[&bind_group_layout],
                depth_format: None,
                sample_count: 1,
            },
        );

        let output = gpu.device.create_texture(&TextureDescriptor {
            label: "output".into(),
            size: Extent3d {
                width: 128,
                height: 128,
                depth_or_array_layers: 6,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: self.format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });

        for (side, bind_group) in bind_groups.iter().enumerate() {
            let view = output.create_view(&TextureViewDescriptor {
                base_array_layer: side as _,
                dimension: Some(TextureViewDimension::D2),
                ..Default::default()
            });

            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: "diffuse_irradiance".into(),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            render_pass.set_pipeline(shader.pipeline());
            render_pass.set_bind_group(0, bind_group, &[]);

            render_pass.draw(0..3, 0..1);
        }

        output
    }
}

fn cube_vertices(gpu: &Gpu) -> TypedBuffer<Vec3> {
    TypedBuffer::new(
        gpu,
        "cube_vertices",
        BufferUsages::VERTEX,
        &[
            Vec3::new(-1.0, -1.0, -1.0),
            Vec3::new(1.0, -1.0, -1.0),
            Vec3::new(1.0, 1.0, -1.0),
            Vec3::new(-1.0, 1.0, -1.0),
            Vec3::new(-1.0, -1.0, 1.0),
            Vec3::new(1.0, -1.0, 1.0),
            Vec3::new(1.0, 1.0, 1.0),
            Vec3::new(-1.0, 1.0, 1.0),
        ],
    )
}

fn cube_indices(gpu: &Gpu) -> TypedBuffer<u16> {
    TypedBuffer::new(
        gpu,
        "cube_indices",
        BufferUsages::INDEX,
        &[
            0, 1, 2, 2, 3, 0, // front
            1, 5, 6, 6, 2, 1, // right
            7, 6, 5, 5, 4, 7, // back
            4, 0, 3, 3, 7, 4, // left
            4, 5, 1, 1, 0, 4, // bottom
            3, 2, 6, 6, 7, 3, // top
        ],
    )
}
