use futures::stream::TryForEachConcurrent;
use glam::{uvec3, vec3};
use ivy_wgpu::{
    rendergraph::{Dependency, Node, TextureHandle},
    types::{BindGroupBuilder, BindGroupLayoutBuilder, TypedBuffer},
    Gpu,
};
use wgpu::{
    BindGroup, BindGroupLayout, BindingType, BufferUsages, ComputePipelineDescriptor,
    PipelineLayoutDescriptor, ShaderStages, StorageTextureAccess, TextureUsages,
};

pub struct MsaaDepthResolve {
    input: TextureHandle,
    output: TextureHandle,
    layout: BindGroupLayout,
    bind_group: Option<BindGroup>,
    pipeline: wgpu::ComputePipeline,
}

impl MsaaDepthResolve {
    pub fn new(gpu: &Gpu, input: TextureHandle, output: TextureHandle) -> Self {
        let layout = BindGroupLayoutBuilder::new("DepthResolve")
            .bind_uniform_buffer(ShaderStages::COMPUTE)
            .bind_texture_depth_multisampled(ShaderStages::COMPUTE)
            .bind(
                ShaderStages::COMPUTE,
                BindingType::StorageTexture {
                    access: StorageTextureAccess::WriteOnly,
                    format: wgpu::TextureFormat::R32Float,
                    view_dimension: wgpu::TextureViewDimension::D2,
                },
            )
            .build(gpu);

        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("DepthResolve"),
                bind_group_layouts: &[&layout],
                push_constant_ranges: &[],
            });

        let pipeline = gpu
            .device
            .create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some("DepthResolve"),
                layout: Some(&pipeline_layout),
                module: &gpu
                    .device
                    .create_shader_module(wgpu::ShaderModuleDescriptor {
                        label: Some("DepthResolve"),
                        source: wgpu::ShaderSource::Wgsl(
                            include_str!("../shaders/depth_resolve.wgsl").into(),
                        ),
                    }),
                entry_point: "main",
                compilation_options: Default::default(),
            });

        Self {
            input,
            output,
            layout,
            bind_group: None,
            pipeline,
        }
    }
}

impl Node for MsaaDepthResolve {
    fn draw(&mut self, ctx: ivy_wgpu::rendergraph::NodeExecutionContext) -> anyhow::Result<()> {
        let input = ctx.get_texture(self.input);
        let output = ctx.get_texture(self.output);

        assert_eq!(input.size(), output.size());

        let bind_group = self.bind_group.get_or_insert_with(|| {
            let buffer = TypedBuffer::new(
                ctx.gpu,
                "DepthResolve",
                BufferUsages::UNIFORM,
                &[uvec3(input.width(), input.height(), input.sample_count())],
            );

            BindGroupBuilder::new("DepthResolve")
                .bind_buffer(&buffer)
                .bind_texture(&input.create_view(&Default::default()))
                .bind_texture(&output.create_view(&Default::default()))
                .build(ctx.gpu, &self.layout)
        });

        let mut compute_pass = ctx
            .encoder
            .begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("DepthResolve"),
                ..Default::default()
            });

        compute_pass.set_pipeline(&self.pipeline);
        compute_pass.set_bind_group(0, bind_group, &[]);

        compute_pass.dispatch_workgroups(input.width(), input.height(), 1);

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
            TextureUsages::STORAGE_BINDING,
        )]
    }
}
