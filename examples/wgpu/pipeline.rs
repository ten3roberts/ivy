use std::{borrow::Cow, path::Path};

use color_eyre::Result;
use wgpu::*;

use crate::Gpu;

pub struct PipelineBuilder<'a> {
    sources: String,
    primitive: PrimitiveState,
    depth_stencil: Option<DepthStencilState>,
    vertex_layouts: Vec<VertexBufferLayout<'a>>,
    multisample: MultisampleState,
}

impl<'a> PipelineBuilder<'a> {
    pub fn new() -> Self {
        Self {
            sources: Default::default(),
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: Some(Face::Back),
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            vertex_layouts: Default::default(),
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
        }
    }

    pub fn with_source(mut self, source: &str) -> Self {
        self.sources.push_str(source.into());
        self
    }

    pub fn with_vertex_layout(mut self, layout: VertexBufferLayout<'a>) -> Self {
        self.vertex_layouts.push(layout);
        self
    }

    pub fn build(self, gpu: &Gpu, label: &str) -> Pipeline {
        let label = Some(label);
        let source = wgpu::ShaderSource::Wgsl(self.sources.into());

        let module = gpu
            .device()
            .create_shader_module(&ShaderModuleDescriptor { label, source });
        let render_pipeline = gpu
            .device()
            .create_render_pipeline(&RenderPipelineDescriptor {
                label,
                layout: None,
                vertex: VertexState {
                    module: &module,
                    entry_point: "vs_main",
                    buffers: &self.vertex_layouts,
                },
                primitive: self.primitive,
                depth_stencil: self.depth_stencil,
                multisample: self.multisample,
                fragment: Some(FragmentState {
                    module: &module,
                    entry_point: "fs_main",
                    targets: &[ColorTargetState {
                        format: gpu.config().format,
                        blend: Some(BlendState::REPLACE),
                        write_mask: ColorWrites::ALL,
                    }],
                }),
                multiview: None,
            });

        Pipeline {
            pipeline: render_pipeline,
        }
    }
}

impl<'a> Default for PipelineBuilder<'a> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct Pipeline {
    pipeline: RenderPipeline,
}

impl Pipeline {
    pub fn builder<'a>() -> PipelineBuilder<'a> {
        PipelineBuilder::default()
    }

    /// Get a reference to the pipeline's pipeline.
    #[must_use]
    pub fn pipeline(&self) -> &RenderPipeline {
        &self.pipeline
    }
}
