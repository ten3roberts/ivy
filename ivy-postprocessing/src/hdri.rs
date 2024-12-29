//! Image based lighting for environment lighting

use std::{any::type_name, mem};

use futures::{stream::BoxStream, FutureExt, StreamExt};
use glam::{Mat4, Vec3};
use image::DynamicImage;
use itertools::Itertools;
use ivy_assets::Asset;
use ivy_core::DEG_90;
use ivy_wgpu::{
    renderer::SkyboxTextures,
    rendergraph::{Dependency, Node, UpdateResult},
    types::{
        shader::{ShaderDesc, TargetDesc},
        BindGroupBuilder, BindGroupLayoutBuilder, PhysicalSize, RenderShader, TypedBuffer,
    },
    Gpu,
};
use wgpu::{
    vertex_attr_array, BufferUsages, Color, CommandEncoder, Extent3d, IndexFormat, LoadOp,
    Operations, RenderPassColorAttachment, RenderPassDescriptor, Sampler, SamplerDescriptor,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp, Texture, TextureDescriptor,
    TextureDimension, TextureFormat, TextureUsages, TextureViewDescriptor, TextureViewDimension,
    VertexBufferLayout, VertexStepMode,
};

pub struct EnvironmentMapMode {}

#[repr(C)]
#[derive(Default, bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Debug)]
struct InverseCameraData {
    inv_proj: Mat4,
    inv_view: Mat4,
}

#[repr(C)]
#[derive(Default, bytemuck::Pod, bytemuck::Zeroable, Clone, Copy, Debug)]
struct ProcessSpecularData {
    inv_proj: Mat4,
    inv_view: Mat4,
    roughness: f32,
    resolution: u32,
    _padding: [f32; 2],
}

pub struct HdriProcessor {
    viewproj_buffers: [TypedBuffer<Mat4>; 6],
    inv_viewproj: [TypedBuffer<InverseCameraData>; 6],
    sampler: Sampler,
    format: TextureFormat,
    // specular_buffers: Vec<TypedBuffer<ProcessSpecularData>>,
    roughness_levels: u32,
    view_matrices: [Mat4; 6],
    proj: Mat4,
}

impl HdriProcessor {
    pub fn new(gpu: &Gpu, format: TextureFormat, roughness_levels: u32) -> Self {
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
            proj,
            viewproj_buffers,
            inv_viewproj,
            sampler,
            format,
            view_matrices,
            // specular_buffers,
            roughness_levels,
        }
    }
    pub fn allocate_cubemap(
        &self,
        gpu: &Gpu,
        extent: PhysicalSize<u32>,
        usage: TextureUsages,
        mip_level_count: u32,
    ) -> Texture {
        gpu.device.create_texture(&TextureDescriptor {
            label: "hdr_cubemap".into(),
            size: Extent3d {
                width: extent.width,
                height: extent.height,
                depth_or_array_layers: 6,
            },
            mip_level_count,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: self.format,
            usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING | usage,
            view_formats: &[],
        })
    }

    pub fn process_hdri(
        &self,
        gpu: &Gpu,
        encoder: &mut CommandEncoder,
        source: &DynamicImage,
        dest: &Texture,
    ) {
        let source_hdri = ivy_wgpu::types::texture::texture_from_image(
            gpu,
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

        let shader = RenderShader::new(
            gpu,
            &ShaderDesc::new(
                "hdri_project",
                &gpu.device.create_shader_module(ShaderModuleDescriptor {
                    label: Some("equirect_project"),
                    source: ShaderSource::Wgsl(
                        include_str!("../shaders/equirect_project.wgsl").into(),
                    ),
                }),
                &TargetDesc {
                    formats: &[self.format],
                    depth_format: None,
                    sample_count: 1,
                },
            )
            .with_vertex_layouts(&[VertexBufferLayout {
                array_stride: 12,
                step_mode: VertexStepMode::Vertex,
                attributes: &vertex_attr_array![0 => Float32x3],
            }])
            .with_bind_group_layouts(&[&bind_group_layout]),
        );

        let vb = cube_vertices(gpu);
        let ib = cube_indices(gpu);

        let sides = (0..6)
            .map(|side| {
                dest.create_view(&TextureViewDescriptor {
                    base_array_layer: side,
                    array_layer_count: Some(1),
                    dimension: Some(TextureViewDimension::D2),
                    mip_level_count: Some(1),
                    ..Default::default()
                })
            })
            .collect_vec();

        for (i, (side, bind_group)) in sides.iter().zip(bind_groups).enumerate() {
            {
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

                render_pass.set_pipeline(shader.pipeline());
                render_pass.set_vertex_buffer(0, vb.slice(..));
                render_pass.set_index_buffer(ib.slice(..), IndexFormat::Uint16);
                render_pass.set_bind_group(0, &bind_group, &[]);

                render_pass.draw_indexed(0..36, 0, 0..1);
            }

            ivy_wgpu::types::mipmap::generate_mipmaps(
                gpu,
                encoder,
                dest,
                dest.mip_level_count(),
                i as u32,
            );
        }
    }

    pub fn process_diffuse_irradiance(
        &self,
        gpu: &Gpu,
        encoder: &mut CommandEncoder,
        hdri: &Texture,
        dest: &Texture,
    ) {
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

        let shader = RenderShader::new(
            gpu,
            &ShaderDesc::new(
                "diffuse_irradiance",
                &gpu.device.create_shader_module(ShaderModuleDescriptor {
                    label: Some("diffuse_irradiance"),
                    source: ShaderSource::Wgsl(
                        include_str!("../shaders/diffuse_irradiance.wgsl").into(),
                    ),
                }),
                &TargetDesc {
                    formats: &[self.format],
                    depth_format: None,
                    sample_count: 1,
                },
            )
            .with_bind_group_layouts(&[&bind_group_layout]),
        );

        for (side, bind_group) in bind_groups.iter().enumerate() {
            let view = dest.create_view(&TextureViewDescriptor {
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
    }

    pub fn process_specular_ibl(
        &self,
        gpu: &Gpu,
        encoder: &mut CommandEncoder,
        hdri: &Texture,
        output: &Texture,
    ) {
        let bind_group_layout = BindGroupLayoutBuilder::new("specular_ibl")
            .bind_uniform_buffer(ShaderStages::FRAGMENT)
            .bind_sampler(ShaderStages::FRAGMENT)
            .bind_texture_cube(ShaderStages::FRAGMENT)
            .build(gpu);

        let specular_buffers = (0..self.roughness_levels)
            .flat_map(|level| {
                let roughness = level as f32 / (self.roughness_levels - 1).max(1) as f32;
                self.view_matrices.map(|v| {
                    TypedBuffer::new(
                        gpu,
                        "diffuse_irradiance",
                        BufferUsages::UNIFORM,
                        &[ProcessSpecularData {
                            inv_proj: self.proj.inverse(),
                            inv_view: v.inverse(),
                            roughness,
                            resolution: hdri.size().width,
                            _padding: [0.0; 2],
                        }],
                    )
                })
            })
            .collect_vec();

        let bind_groups = specular_buffers
            .iter()
            .map(|v| {
                BindGroupBuilder::new("specular_ibl")
                    .bind_buffer(v.buffer())
                    .bind_sampler(&self.sampler)
                    .bind_texture(&hdri.create_view(&TextureViewDescriptor {
                        dimension: Some(TextureViewDimension::Cube),
                        ..Default::default()
                    }))
                    .build(gpu, &bind_group_layout)
            })
            .collect_vec();

        let shader = RenderShader::new(
            gpu,
            &ShaderDesc::new(
                "specular_ibl",
                &gpu.device.create_shader_module(ShaderModuleDescriptor {
                    label: Some("specular_ibl"),
                    source: ShaderSource::Wgsl(include_str!("../shaders/specular_ibl.wgsl").into()),
                }),
                &TargetDesc {
                    formats: &[self.format],
                    depth_format: None,
                    sample_count: 1,
                },
            )
            .with_bind_group_layouts(&[&bind_group_layout]),
        );

        for mip_level in 0..self.roughness_levels {
            for side in 0..6 {
                let bind_group = &bind_groups[(side + (mip_level * 6)) as usize];

                let view = output.create_view(&TextureViewDescriptor {
                    base_array_layer: side as _,
                    dimension: Some(TextureViewDimension::D2),
                    base_mip_level: mip_level,
                    mip_level_count: Some(1),
                    ..Default::default()
                });

                let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                    label: "specular_ibl".into(),
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
        }
    }

    pub fn process_brdf_lookup(&self, gpu: &Gpu, encoder: &mut CommandEncoder, dest: &Texture) {
        let shader = RenderShader::new(
            gpu,
            &ShaderDesc::new(
                "brdf_lookup",
                &gpu.device.create_shader_module(ShaderModuleDescriptor {
                    label: Some("brdf_lookup"),
                    source: ShaderSource::Wgsl(include_str!("../shaders/brdf_lookup.wgsl").into()),
                }),
                &TargetDesc {
                    formats: &[self.format],
                    depth_format: None,
                    sample_count: 1,
                },
            ),
        );

        let view = dest.create_view(&TextureViewDescriptor {
            ..Default::default()
        });

        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: "brdf_lookup".into(),
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

        render_pass.draw(0..3, 0..1);
    }

    pub fn format(&self) -> TextureFormat {
        self.format
    }
}

pub struct HdriProcessorNode {
    processor: HdriProcessor,
    skybox: SkyboxTextures,
    incoming: futures::stream::Fuse<BoxStream<'static, Asset<DynamicImage>>>,
    source: Option<Asset<DynamicImage>>,
    process_hdri: bool,
    process_brdf_lookup: bool,
}

impl HdriProcessorNode {
    pub fn new(
        processor: HdriProcessor,
        source: BoxStream<'static, Asset<DynamicImage>>,
        skybox: SkyboxTextures,
    ) -> Self {
        Self {
            processor,
            incoming: source.fuse(),
            skybox,
            process_brdf_lookup: true,
            source: None,
            process_hdri: true,
        }
    }
}

impl Node for HdriProcessorNode {
    fn label(&self) -> &str {
        type_name::<Self>()
    }

    fn update(
        &mut self,
        _ctx: ivy_wgpu::rendergraph::NodeUpdateContext,
    ) -> anyhow::Result<UpdateResult> {
        if let Some(source) = self.incoming.next().now_or_never().flatten() {
            self.process_hdri = true;
            self.source = Some(source);
        }

        Ok(UpdateResult::Success)
    }

    fn draw(&mut self, ctx: ivy_wgpu::rendergraph::NodeExecutionContext) -> anyhow::Result<()> {
        if let Some(source) = &self.source {
            if mem::take(&mut self.process_hdri) {
                let environment_map = ctx.get_texture(self.skybox.environment_map);
                tracing::info!(
                    "processing hdri {:?} {:?}",
                    environment_map.size(),
                    self.skybox.environment_map
                );
                self.processor
                    .process_hdri(ctx.gpu, ctx.encoder, source, environment_map);

                self.processor.process_diffuse_irradiance(
                    ctx.gpu,
                    ctx.encoder,
                    environment_map,
                    ctx.get_texture(self.skybox.irradiance_map),
                );

                self.processor.process_specular_ibl(
                    ctx.gpu,
                    ctx.encoder,
                    environment_map,
                    ctx.get_texture(self.skybox.specular_map),
                );
            }
        }

        if mem::take(&mut self.process_brdf_lookup) {
            self.processor.process_brdf_lookup(
                ctx.gpu,
                ctx.encoder,
                ctx.get_texture(self.skybox.integrated_brdf),
            );
        }
        Ok(())
    }

    fn read_dependencies(&self) -> Vec<ivy_wgpu::rendergraph::Dependency> {
        vec![]
    }

    fn write_dependencies(&self) -> Vec<ivy_wgpu::rendergraph::Dependency> {
        vec![
            Dependency::texture(
                self.skybox.environment_map,
                TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
            ),
            Dependency::texture(self.skybox.irradiance_map, TextureUsages::RENDER_ATTACHMENT),
            Dependency::texture(self.skybox.specular_map, TextureUsages::RENDER_ATTACHMENT),
            Dependency::texture(
                self.skybox.integrated_brdf,
                TextureUsages::RENDER_ATTACHMENT,
            ),
        ]
    }

    fn on_resource_changed(&mut self, _resource: ivy_wgpu::rendergraph::ResourceHandle) {
        tracing::info!("reprocessing");
        self.process_hdri = true;
        self.process_brdf_lookup = true;
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
