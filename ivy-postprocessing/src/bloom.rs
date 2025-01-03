use std::slice;

use glam::{uvec2, vec3};
use itertools::Itertools;
use ivy_core::profiling::{profile_function, profile_scope};
use ivy_wgpu::{
    rendergraph::{Dependency, Node, TextureHandle},
    types::{
        shader::{ShaderDesc, TargetDesc},
        BindGroupBuilder, BindGroupLayoutBuilder, RenderShader, TypedBuffer,
    },
    Gpu,
};
use wgpu::{
    BindGroup, BindGroupLayout, BufferUsages, Color, Operations, RenderPassColorAttachment,
    Sampler, SamplerDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp,
    TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
    TextureViewDescriptor,
};

struct Data {
    bind_groups: Vec<BindGroup>,
    mip_chain: Vec<TextureView>,
    mix_bind_group: BindGroup,
}

pub struct BloomNode {
    input: TextureHandle,
    final_output: TextureHandle,

    layout: BindGroupLayout,
    mix_layout: BindGroupLayout,
    mip_level_count: u32,
    data: Option<Data>,

    downsample_shader: RenderShader,
    upsample_shader: RenderShader,
    mix_shader: RenderShader,

    sampler: Sampler,
    filter_radius: f32,
}

impl BloomNode {
    pub fn new(
        gpu: &Gpu,
        input: TextureHandle,
        output: TextureHandle,
        mip_level_count: u32,
        filter_radius: f32,
    ) -> Self {
        let layout = BindGroupLayoutBuilder::new("Bloom")
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_sampler(ShaderStages::FRAGMENT)
            .bind_uniform_buffer(ShaderStages::FRAGMENT)
            .build(gpu);

        let downsample_shader = RenderShader::new(
            gpu,
            &ShaderDesc::new(
                "bloom_downsample",
                &gpu.device.create_shader_module(ShaderModuleDescriptor {
                    label: Some("bloom_downsample"),
                    source: ShaderSource::Wgsl(
                        include_str!("../shaders/bloom_downsample.wgsl").into(),
                    ),
                }),
                &TargetDesc {
                    formats: &[TextureFormat::Rgba16Float],
                    depth_format: None,
                    sample_count: 1,
                },
            )
            .with_bind_group_layouts(&[&layout]),
        );

        let upsample_shader = RenderShader::new(
            gpu,
            &ShaderDesc::new(
                "bloom_upsample",
                &gpu.device.create_shader_module(ShaderModuleDescriptor {
                    label: Some("bloom_upsample"),
                    source: ShaderSource::Wgsl(
                        include_str!("../shaders/bloom_upsample.wgsl").into(),
                    ),
                }),
                &TargetDesc {
                    formats: &[TextureFormat::Rgba16Float],
                    depth_format: None,
                    sample_count: 1,
                },
            )
            .with_bind_group_layouts(&[&layout]),
        );

        let mix_layout = BindGroupLayoutBuilder::new("bloom_mix")
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_sampler(ShaderStages::FRAGMENT)
            .build(gpu);

        let mix_shader = RenderShader::new(
            gpu,
            &ShaderDesc::new(
                "bloom_mix",
                &gpu.device.create_shader_module(ShaderModuleDescriptor {
                    label: Some("bloom_mix"),
                    source: ShaderSource::Wgsl(include_str!("../shaders/bloom_mix.wgsl").into()),
                }),
                &TargetDesc {
                    formats: &[TextureFormat::Rgba16Float],
                    depth_format: None,
                    sample_count: 1,
                },
            )
            .with_bind_group_layouts(&[&mix_layout]),
        );
        let sampler = gpu.device.create_sampler(&SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        Self {
            input,
            layout,
            mix_layout,
            final_output: output,
            data: None,
            mip_level_count,
            downsample_shader,
            mix_shader,
            upsample_shader,
            sampler,
            filter_radius,
        }
    }
}

impl Node for BloomNode {
    fn draw(&mut self, ctx: ivy_wgpu::rendergraph::NodeExecutionContext) -> anyhow::Result<()> {
        profile_function!();
        let input = ctx.get_texture(self.input);
        let output = ctx.get_texture(self.final_output);

        let output_view = output.create_view(&Default::default());

        let data = self.data.get_or_insert_with(|| {
            let mip_texture = ctx.gpu.device.create_texture(&TextureDescriptor {
                label: "bloom_mips".into(),
                size: wgpu::Extent3d {
                    width: input.width() / 2,
                    height: input.height() / 2,
                    depth_or_array_layers: 1,
                },
                mip_level_count: self.mip_level_count,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rgba16Float,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });

            let mip_chain = (0..self.mip_level_count)
                .map(|level| {
                    mip_texture.create_view(&TextureViewDescriptor {
                        base_mip_level: level,
                        mip_level_count: Some(1),
                        ..Default::default()
                    })
                })
                .collect_vec();

            // input, mip, ... mip
            let input_view = input.create_view(&Default::default());
            let views = [
                slice::from_ref(&input_view),
                &mip_chain,
                // slice::from_ref(&output_view),
            ];

            let mut size = uvec2(input.width(), input.height());
            let bind_groups = {
                profile_scope!("create_bind_groups");
                views
                    .into_iter()
                    .flatten()
                    .map(|view| {
                        let uniform_data = TypedBuffer::new(
                            ctx.gpu,
                            "Bloom",
                            BufferUsages::UNIFORM,
                            &[vec3(
                                1.0 / (size.x as f32),
                                1.0 / (size.y as f32),
                                self.filter_radius,
                            )],
                        );

                        size /= 2;

                        BindGroupBuilder::new("Bloom")
                            .bind_texture(view)
                            .bind_sampler(&self.sampler)
                            .bind_buffer(&uniform_data)
                            .build(ctx.gpu, &self.layout)
                    })
                    .collect_vec()
            };

            let mix_bind_group = BindGroupBuilder::new("bloom_mix")
                .bind_texture(&input_view)
                .bind_texture(&mip_chain[0])
                .bind_sampler(&self.sampler)
                .build(ctx.gpu, &self.mix_layout);

            Data {
                bind_groups,
                mix_bind_group,
                mip_chain,
            }
        });

        for (bind_group, target) in data.bind_groups[..data.bind_groups.len() - 1]
            .iter()
            .zip(&data.mip_chain)
        {
            profile_scope!("downsample");
            let mut render_pass = {
                profile_scope!("begin_render_pass");
                ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: "Bloom".into(),
                    color_attachments: &[Some(RenderPassColorAttachment {
                        view: target,
                        resolve_target: None,
                        ops: Operations {
                            load: wgpu::LoadOp::Clear(Color::BLACK),
                            store: StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                })
            };

            {
                profile_scope!("set_pipeline");
                render_pass.set_pipeline(self.downsample_shader.pipeline());
                render_pass.set_bind_group(0, bind_group, &[]);
            }

            {
                profile_scope!("draw");
                render_pass.draw(0..3, 0..1);
            }
            {
                profile_scope!("drop");
                drop(render_pass);
            }
        }

        // tracing::info!("upsampling");

        for (bind_group, target) in
            // skip input and last mip
            data.bind_groups[2..]
                .iter()
                .rev()
                .zip_eq(data.mip_chain.iter().rev().skip(1))
        {
            profile_scope!("upsample");
            let mut render_pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: "Bloom".into(),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: Operations {
                        load: wgpu::LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            render_pass.set_pipeline(self.upsample_shader.pipeline());
            render_pass.set_bind_group(0, bind_group, &[]);

            render_pass.draw(0..3, 0..1);
        }

        {
            profile_scope!("mix");
            let mut render_pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: "Bloom".into(),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &output_view,
                    resolve_target: None,
                    ops: Operations {
                        load: wgpu::LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            render_pass.set_pipeline(self.mix_shader.pipeline());
            render_pass.set_bind_group(0, &data.mix_bind_group, &[]);

            render_pass.draw(0..3, 0..1);
        }

        Ok(())
    }

    fn on_resource_changed(&mut self, _resource: ivy_wgpu::rendergraph::ResourceHandle) {
        self.data = None;
    }

    fn read_dependencies(&self) -> Vec<Dependency> {
        vec![Dependency::texture(
            self.input,
            TextureUsages::TEXTURE_BINDING,
        )]
    }

    fn write_dependencies(&self) -> Vec<Dependency> {
        vec![Dependency::texture(
            self.final_output,
            TextureUsages::RENDER_ATTACHMENT,
        )]
    }
}
