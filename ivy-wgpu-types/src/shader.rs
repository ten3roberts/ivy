use crate::Gpu;
use wgpu::{
    BindGroupLayout, PipelineLayoutDescriptor, RenderPipeline, TextureFormat, VertexBufferLayout,
};

#[derive(Debug, Clone)]
pub struct ShaderDesc<'a> {
    pub label: &'a str,
    pub source: &'a str,
    pub format: TextureFormat,
    pub vertex_layouts: &'a [VertexBufferLayout<'static>],
    pub layouts: &'a [&'a BindGroupLayout],
    pub depth_format: Option<TextureFormat>,
    pub sample_count: u32,
    pub vertex_entry_point: &'a str,
    pub fragment_entry_point: &'a str,
}

/// Represents a graphics shader
#[derive(Debug)]
pub struct Shader {
    pipeline: RenderPipeline,
}

impl Shader {
    pub fn new(gpu: &Gpu, desc: &ShaderDesc) -> Self {
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(desc.label),
                source: wgpu::ShaderSource::Wgsl(desc.source.into()),
            });

        let layout = gpu
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some(desc.label),
                bind_group_layouts: desc.layouts,
                push_constant_ranges: &[],
            });

        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(desc.label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: desc.vertex_entry_point,
                    buffers: desc.vertex_layouts,
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    // 3.
                    module: &shader,
                    entry_point: desc.fragment_entry_point,
                    targets: &[Some(wgpu::ColorTargetState {
                        // 4.
                        format: desc.format,
                        blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList, // 1.
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Cw, // 2.
                    cull_mode: None,                 //Some(wgpu::Face::Back),
                    // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                    polygon_mode: wgpu::PolygonMode::Fill,
                    // Requires Features::DEPTH_CLIP_CONTROL
                    unclipped_depth: false,
                    // Requires Features::CONSERVATIVE_RASTERIZATION
                    conservative: false,
                },
                depth_stencil: desc.depth_format.map(|format| wgpu::DepthStencilState {
                    format,
                    depth_write_enabled: true,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState {
                    count: desc.sample_count,         // 2.
                    mask: !0,                         // 3.
                    alpha_to_coverage_enabled: false, // 4.
                },
                multiview: None,
            });

        Self { pipeline }
    }

    pub fn pipeline(&self) -> &RenderPipeline {
        &self.pipeline
    }
}
