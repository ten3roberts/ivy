use ivy_wgpu::{
    rendergraph::{Dependency, Node, TextureHandle},
    types::{
        shader::{ShaderDesc, TargetDesc},
        BindGroupBuilder, BindGroupLayoutBuilder, RenderShader,
    },
    Gpu,
};
use wgpu::{
    BindGroup, BindGroupLayout, BindingType, Operations, RenderPassColorAttachment,
    SamplerDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp,
    TextureSampleType, TextureUsages, TextureViewDimension,
};

pub struct OverlayNode {
    input: TextureHandle,
    output: TextureHandle,
    shader: Option<RenderShader>,
    layout: BindGroupLayout,
    bind_group: Option<BindGroup>,
    default_sampler: wgpu::Sampler,
}

impl OverlayNode {
    pub fn new(gpu: &Gpu, input: TextureHandle, output: TextureHandle) -> Self {
        let layout = BindGroupLayoutBuilder::new("Overlay")
            .bind(
                ShaderStages::FRAGMENT,
                BindingType::Texture {
                    sample_type: TextureSampleType::Float { filterable: false },
                    view_dimension: TextureViewDimension::D2,
                    multisampled: false,
                },
            )
            .bind_sampler_nonfiltering(ShaderStages::FRAGMENT)
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

impl Node for OverlayNode {
    fn draw(&mut self, ctx: ivy_wgpu::rendergraph::NodeExecutionContext) -> anyhow::Result<()> {
        let input = ctx.get_texture(self.input);
        let output = ctx.get_texture(self.output);

        let bind_group = BindGroupBuilder::new("Overlay")
            .bind_texture(&input.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(TextureViewDimension::D2),
                base_array_layer: 0,
                array_layer_count: Some(1),
                ..Default::default()
            }))
            .bind_sampler(&self.default_sampler)
            .build(ctx.gpu, &self.layout);

        let shader = self.shader.get_or_insert_with(|| {
            RenderShader::new(
                ctx.gpu,
                &ShaderDesc::new(
                    "overlay",
                    &ctx.gpu.device.create_shader_module(ShaderModuleDescriptor {
                        label: Some("overlay"),
                        source: ShaderSource::Wgsl(include_str!("../shaders/overlay.wgsl").into()),
                    }),
                    &TargetDesc {
                        formats: &[output.format()],
                        depth_format: None,
                        sample_count: 1,
                    },
                )
                .with_bind_group_layouts(&[&self.layout]),
            )
        });

        let output_view = output.create_view(&Default::default());
        let mut render_pass = ctx.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: "Overlay".into(),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: &output_view,
                resolve_target: None,
                ops: Operations {
                    load: wgpu::LoadOp::Load,
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            ..Default::default()
        });

        render_pass.set_pipeline(shader.pipeline());
        render_pass.set_bind_group(0, &bind_group, &[]);

        render_pass.draw(0..3, 0..1);

        Ok(())
    }

    fn read_dependencies(&self) -> Vec<ivy_wgpu::rendergraph::Dependency> {
        vec![
            Dependency::texture(self.input, TextureUsages::TEXTURE_BINDING),
            Dependency::texture(self.output, TextureUsages::RENDER_ATTACHMENT),
        ]
    }

    fn write_dependencies(&self) -> Vec<ivy_wgpu::rendergraph::Dependency> {
        vec![]
    }

    fn on_resource_changed(&mut self, _resource: ivy_wgpu::rendergraph::ResourceHandle) {
        self.bind_group = None;
    }
}
