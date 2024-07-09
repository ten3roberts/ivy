use std::{any::type_name, process::Output, slice};

use glam::{vec2, Vec2};
use itertools::Itertools;
use ivy_wgpu::{
    rendergraph::{Dependency, Node, TextureDesc, TextureHandle},
    types::{shader::ShaderDesc, BindGroupBuilder, BindGroupLayoutBuilder, Shader, TypedBuffer},
    Gpu,
};
use wgpu::{
    util::RenderEncoder, BindGroup, BindGroupLayout, BufferUsages, Color, Operations,
    RenderPassColorAttachment, ShaderStages, StoreOp, Texture, TextureDescriptor, TextureDimension,
    TextureFormat, TextureUsages, TextureView, TextureViewDescriptor,
};

pub struct BloomNode {
    input: TextureHandle,
    layout: BindGroupLayout,
    final_output: TextureHandle,
    bind_groups: Option<Vec<BindGroup>>,
    mip_level_count: u32,

    downsample_shader: Shader,
    upsample_shader: Shader,
    uniform_buffer: TypedBuffer<Vec2>,
}

impl BloomNode {
    pub fn new(
        gpu: &Gpu,
        input: TextureHandle,
        output: TextureHandle,
        mip_level_count: u32,
    ) -> Self {
        let layout = BindGroupLayoutBuilder::new("Bloom")
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_uniform_buffer(ShaderStages::FRAGMENT)
            .build(gpu);

        let uniform_buffer =
            TypedBuffer::new(gpu, "Bloom", BufferUsages::UNIFORM, &[Default::default()]);

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
                label: "bloom_downsample",
                source: include_str!("../shaders/bloom_upsample.wgsl"),
                format: TextureFormat::Rgba16Float,
                vertex_layouts: &[],
                layouts: &[&layout],
                depth_format: None,
                sample_count: 1,
            },
        );

        Self {
            input,
            layout,
            final_output: output,
            bind_groups: None,
            mip_level_count,
            downsample_shader,
            upsample_shader,
            uniform_buffer,
        }
    }
}

impl Node for BloomNode {
    fn draw(&mut self, ctx: ivy_wgpu::rendergraph::NodeExecutionContext) -> anyhow::Result<()> {
        let input = ctx.get_texture(self.input);
        let output = ctx.get_texture(self.final_output);

        self.uniform_buffer.write(
            ctx.queue,
            0,
            &[vec2(input.size().width as f32, input.size().height as f32)],
        );

        let mip_texture = ctx.gpu.device.create_texture(&TextureDescriptor {
            label: "bloom_mips".into(),
            size: input.size(),
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

        // input, mip, ... mip, output
        let bind_groups = self.bind_groups.get_or_insert_with(|| {
            let input_view = input.create_view(&Default::default());
            let output_view = output.create_view(&Default::default());
            let views = [
                slice::from_ref(&input_view),
                &mip_chain,
                slice::from_ref(&output_view),
            ];

            views
                .into_iter()
                .flatten()
                .map(|view| {
                    BindGroupBuilder::new("Bloom")
                        .bind_texture(view)
                        .bind_buffer(&self.uniform_buffer)
                        .build(ctx.gpu, &self.layout)
                })
                .collect_vec()
        });

        for (bind_group, target) in bind_groups[0..].iter().zip_eq(&mip_chain) {
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

        for (bind_group, target) in bind_groups[1..].iter().zip_eq(&mip_chain) {
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

        Ok(())
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
