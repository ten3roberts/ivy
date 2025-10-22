use glam::Vec3;
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
    SamplerDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp, TextureUsages,
};

use crate::preconfigured::pbr::ColorGradingConfig;

pub struct TonemapNode {
    input: TextureHandle,
    output: TextureHandle,
    shader: Option<RenderShader>,
    layout: BindGroupLayout,
    bind_group: Option<BindGroup>,
    default_sampler: wgpu::Sampler,

    // Color grading support
    grading_layout: BindGroupLayout,
    grading_bind_group: Option<BindGroup>,
    grading_buffer: Option<TypedBuffer<ColorGradingConfig>>,
    current_grading: ColorGradingConfig,
}

impl TonemapNode {
    pub fn new(gpu: &Gpu, input: TextureHandle, output: TextureHandle) -> Self {
        Self::new_with_grading(gpu, input, output, ColorGradingConfig::default())
    }

    pub fn new_with_grading(
        gpu: &Gpu,
        input: TextureHandle,
        output: TextureHandle,
        grading: ColorGradingConfig,
    ) -> Self {
        let layout = BindGroupLayoutBuilder::new("Tonemap")
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_sampler(ShaderStages::FRAGMENT)
            .build(gpu);

        let grading_layout = BindGroupLayoutBuilder::new("ColorGrading")
            .bind_uniform_buffer(ShaderStages::FRAGMENT)
            .build(gpu);

        let default_sampler = gpu.device.create_sampler(&SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            input,
            output,
            shader: None,
            bind_group: None,
            layout,
            default_sampler,
            grading_layout,
            grading_bind_group: None,
            grading_buffer: None,
            current_grading: grading,
        }
    }

    pub fn update_grading(&mut self, grading: &ColorGradingConfig) {
        self.current_grading = *grading;
        self.grading_bind_group = None; // Force recreation
        self.grading_buffer = None;
    }
}

impl Node for TonemapNode {
    fn draw(&mut self, ctx: ivy_wgpu::rendergraph::NodeExecutionContext) -> anyhow::Result<()> {
        let input = ctx.get_texture(self.input);
        let output = ctx.get_texture(self.output);

        let bind_group = self.bind_group.get_or_insert_with(|| {
            BindGroupBuilder::new("Tonemap")
                .bind_texture(&input.create_view(&Default::default()))
                .bind_sampler(&self.default_sampler)
                .build(ctx.gpu, &self.layout)
        });

        let grading_buffer = self.grading_buffer.get_or_insert_with(|| {
            TypedBuffer::new(
                ctx.gpu,
                "ColorGrading",
                BufferUsages::UNIFORM,
                &[self.current_grading],
            )
        });

        let grading_bind_group = self.grading_bind_group.get_or_insert_with(|| {
            BindGroupBuilder::new("ColorGrading")
                .bind_buffer(grading_buffer)
                .build(ctx.gpu, &self.grading_layout)
        });

        let shader = self.shader.get_or_insert_with(|| {
            RenderShader::new(
                ctx.gpu,
                &ShaderDesc::new(
                    "tonemap",
                    &ctx.gpu.device.create_shader_module(ShaderModuleDescriptor {
                        label: Some("tonemap"),
                        source: ShaderSource::Wgsl(include_str!("../shaders/tonemap.wgsl").into()),
                    }),
                    &TargetDesc {
                        formats: &[output.format()],
                        depth_format: None,
                        sample_count: 1,
                    },
                )
                .with_bind_group_layouts(&[&self.layout, &self.grading_layout]),
            )
        });

        let output_view = output.create_view(&Default::default());
        let mut render_pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: "Tonemap".into(),
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

        render_pass.set_pipeline(shader.pipeline());
        render_pass.set_bind_group(0, bind_group, &[]);
        render_pass.set_bind_group(1, grading_bind_group, &[]);

        render_pass.draw(0..3, 0..1);

        Ok(())
    }

    fn read_dependencies(&self) -> Vec<ivy_wgpu::rendergraph::Dependency> {
        vec![Dependency::texture(
            self.input,
            TextureUsages::TEXTURE_BINDING,
        )]
    }

    fn write_dependencies(&self) -> Vec<ivy_wgpu::rendergraph::Dependency> {
        vec![Dependency::texture(
            self.output,
            TextureUsages::RENDER_ATTACHMENT,
        )]
    }

    fn on_resource_changed(&mut self, _resource: ivy_wgpu::rendergraph::ResourceHandle) {
        self.bind_group = None;
        self.grading_bind_group = None;
    }
}
