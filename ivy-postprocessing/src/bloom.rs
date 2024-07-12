use std::{any::type_name, process::Output, slice};

use glam::{ivec2, uvec2, vec2, vec3, Vec2};
use itertools::Itertools;
use ivy_core::palette::num::Recip;
use ivy_wgpu::{
    rendergraph::{Dependency, Node, TextureDesc, TextureHandle},
    types::{shader::ShaderDesc, BindGroupBuilder, BindGroupLayoutBuilder, Shader, TypedBuffer},
    Gpu,
};
use wgpu::{
    hal::BufferUses, util::RenderEncoder, BindGroup, BindGroupLayout, BufferUsages, Color,
    Operations, RenderPassColorAttachment, Sampler, SamplerDescriptor, ShaderStages, StoreOp,
    Texture, TextureDescriptor, TextureDimension, TextureFormat, TextureUsages, TextureView,
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

    downsample_shader: Shader,
    upsample_shader: Shader,
    mix_shader: Shader,

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

        let downsample_shader = Shader::new(
            gpu,
            &ShaderDesc {
                label: "bloom_downsample",
                source: include_str!("../shaders/bloom_downsample.wgsl"),
                format: TextureFormat::Rgba16Float,
                vertex_layouts: &[],
                layouts: &[&layout],
                depth_format: None,
                sample_count: 1,
            },
        );

        let upsample_shader = Shader::new(
            gpu,
            &ShaderDesc {
                label: "bloom_upsample",
                source: include_str!("../shaders/bloom_upsample.wgsl"),
                format: TextureFormat::Rgba16Float,
                vertex_layouts: &[],
                layouts: &[&layout],
                depth_format: None,
                sample_count: 1,
            },
        );

        let mix_layout = BindGroupLayoutBuilder::new("bloom_mix")
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_sampler(ShaderStages::FRAGMENT)
            .build(gpu);

        let mix_shader = Shader::new(
            gpu,
            &ShaderDesc {
                label: "bloom_mix",
                source: include_str!("../shaders/bloom_mix.wgsl"),
                format: TextureFormat::Rgba16Float,
                vertex_layouts: &[],
                layouts: &[&mix_layout],
                depth_format: None,
                sample_count: 1,
            },
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
            let bind_groups = views
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
                .collect_vec();

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
        // tracing::info!("downsampling");

        for (bind_group, target) in data.bind_groups[..data.bind_groups.len() - 1]
            .iter()
            .zip_eq(&data.mip_chain)
        {
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

            render_pass.set_pipeline(self.downsample_shader.pipeline());
            render_pass.set_bind_group(0, bind_group, &[]);

            render_pass.draw(0..3, 0..1);
        }

        // tracing::info!("upsampling");

        for (bind_group, target) in
            // skip input and last mip
            data.bind_groups[2..]
                .iter()
                .rev()
                .zip_eq(data.mip_chain.iter().rev().skip(1))
        {
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
