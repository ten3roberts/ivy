use ivy_wgpu::{
    rendergraph::{Dependency, Node, TextureHandle},
    types::{
        shader::{ShaderDesc, TargetDesc},
        BindGroupBuilder, BindGroupLayoutBuilder, Shader,
    },
    Gpu,
};
use wgpu::{
    BindGroup, BindGroupLayout, Color, Operations, RenderPassColorAttachment, SamplerDescriptor,
    ShaderStages, StoreOp, TextureFormat, TextureUsages,
};

pub struct TonemapNode {
    input: TextureHandle,
    output: TextureHandle,
    shader: Option<Shader>,
    layout: BindGroupLayout,
    bind_group: Option<BindGroup>,
    default_sampler: wgpu::Sampler,
}

impl TonemapNode {
    pub fn new(gpu: &Gpu, input: TextureHandle, output: TextureHandle) -> Self {
        let layout = BindGroupLayoutBuilder::new("Tonemap")
            .bind_texture(ShaderStages::FRAGMENT)
            .bind_sampler(ShaderStages::FRAGMENT)
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
        }
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

        let shader = self.shader.get_or_insert_with(|| {
            Shader::new(
                ctx.gpu,
                &ShaderDesc {
                    label: "tonemap",
                    source: include_str!("../shaders/tonemap.wgsl"),
                    target: &TargetDesc {
                        formats: &[output.format()],
                        depth_format: None,
                        sample_count: 1,
                    },
                    vertex_layouts: &[],
                    layouts: &[&self.layout],
                    fragment_entry_point: "fs_main",
                    vertex_entry_point: "vs_main",
                },
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
    }
}
