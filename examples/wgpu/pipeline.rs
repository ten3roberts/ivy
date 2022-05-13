use std::path::Path;

use color_eyre::Result;
use wgpu::*;

use crate::Gpu;

pub struct Pipeline {
    pipeline: RenderPipeline,
}

impl Pipeline {
    pub async fn from_path(gpu: &Gpu, path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let label = path.to_string_lossy();
        let label = Some(&*label);
        let source = wgpu::ShaderSource::Wgsl(tokio::fs::read_to_string(path).await?.into());

        let shader = gpu
            .device()
            .create_shader_module(&ShaderModuleDescriptor { label, source });
        let render_pipeline =
            gpu.device()
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label,
                    layout: None,
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: "vs_main",
                        buffers: &[],
                    },
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
                    multisample: MultisampleState {
                        count: 1,
                        ..Default::default()
                    },
                    fragment: Some(FragmentState {
                        module: &shader,
                        entry_point: "fs_main",
                        targets: &[ColorTargetState {
                            format: gpu.config.format,
                            blend: Some(BlendState::REPLACE),
                            write_mask: ColorWrites::ALL,
                        }],
                    }),
                    multiview: None,
                });

        Ok(Self {
            pipeline: render_pipeline,
        })
    }

    /// Get a reference to the pipeline's pipeline.
    #[must_use]
    pub fn pipeline(&self) -> &RenderPipeline {
        &self.pipeline
    }
}
