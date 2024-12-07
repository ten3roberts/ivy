use itertools::Itertools;
use wgpu::{
    BindGroupLayout, DepthBiasState, Face, FrontFace, PipelineLayoutDescriptor, RenderPipeline,
    TextureFormat, VertexBufferLayout,
};

use crate::Gpu;

#[derive(Debug, Clone, Hash, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Culling {
    pub cull_mode: Option<Face>,
    pub front_face: FrontFace,
}

impl Default for Culling {
    fn default() -> Self {
        Self {
            cull_mode: None,
            front_face: FrontFace::Ccw,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TargetDesc<'a> {
    pub formats: &'a [TextureFormat],
    pub depth_format: Option<TextureFormat>,
    pub sample_count: u32,
}

#[derive(Debug)]
pub struct ShaderDesc<'a> {
    pub label: &'a str,
    pub module: &'a wgpu::ShaderModule,
    pub target: &'a TargetDesc<'a>,
    pub vertex_layouts: &'a [VertexBufferLayout<'static>],
    pub bind_group_layouts: &'a [&'a BindGroupLayout],
    pub vertex_entry_point: &'a str,
    pub fragment_entry_point: &'a str,
    pub culling_mode: Culling,
    pub depth_bias: DepthBiasState,
}

impl<'a> ShaderDesc<'a> {
    pub fn new(label: &'a str, module: &'a wgpu::ShaderModule, target: &'a TargetDesc<'a>) -> Self {
        Self {
            label,
            module,
            target,
            vertex_layouts: &[],
            bind_group_layouts: &[],
            vertex_entry_point: "vs_main",
            fragment_entry_point: "fs_main",
            culling_mode: Default::default(),
            depth_bias: Default::default(),
        }
    }

    /// Set the vertex layouts
    pub fn with_vertex_layouts(
        mut self,
        vertex_layouts: &'a [VertexBufferLayout<'static>],
    ) -> Self {
        self.vertex_layouts = vertex_layouts;
        self
    }

    /// Set the bind group layout
    pub fn with_bind_group_layouts(
        mut self,
        bind_group_layouts: &'a [&'a BindGroupLayout],
    ) -> Self {
        self.bind_group_layouts = bind_group_layouts;
        self
    }

    /// Set the depth bias
    pub fn with_depth_bias(mut self, depth_bias: DepthBiasState) -> Self {
        self.depth_bias = depth_bias;
        self
    }

    /// Set the culling mode
    pub fn with_culling_mode(mut self, culling_mode: Culling) -> Self {
        self.culling_mode = culling_mode;
        self
    }
}

/// Represents a graphics shader
#[derive(Debug)]
pub struct Shader {
    pipeline: RenderPipeline,
}

impl Shader {
    pub fn new(gpu: &Gpu, desc: &ShaderDesc) -> Self {
        // let shader = gpu
        //     .device
        //     .create_shader_module(wgpu::ShaderModuleDescriptor {
        //         label: Some(desc.label),
        //         source: wgpu::ShaderSource::Wgsl(desc.source.into()),
        //     });

        let layout = gpu
            .device
            .create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some(desc.label),
                bind_group_layouts: desc.bind_group_layouts,
                push_constant_ranges: &[],
            });

        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(desc.label),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: desc.module,
                    entry_point: desc.vertex_entry_point,
                    buffers: desc.vertex_layouts,
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    // 3.
                    module: desc.module,
                    entry_point: desc.fragment_entry_point,
                    targets: &desc
                        .target
                        .formats
                        .iter()
                        .map(|&format| {
                            Some(wgpu::ColorTargetState {
                                // 4.
                                format,
                                blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                                write_mask: wgpu::ColorWrites::ALL,
                            })
                        })
                        .collect_vec(),
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList, // 1.
                    strip_index_format: None,
                    front_face: desc.culling_mode.front_face,
                    cull_mode: desc.culling_mode.cull_mode,
                    // Setting this to anything other than Fill requires Features::NON_FILL_POLYGON_MODE
                    polygon_mode: wgpu::PolygonMode::Fill,
                    // Requires Features::DEPTH_CLIP_CONTROL
                    unclipped_depth: false,
                    // Requires Features::CONSERVATIVE_RASTERIZATION
                    conservative: false,
                },
                depth_stencil: desc
                    .target
                    .depth_format
                    .map(|format| wgpu::DepthStencilState {
                        format,
                        depth_write_enabled: true,
                        depth_compare: wgpu::CompareFunction::LessEqual,
                        stencil: Default::default(),
                        bias: desc.depth_bias,
                    }),
                multisample: wgpu::MultisampleState {
                    count: desc.target.sample_count,  // 2.
                    mask: !0,                         // 3.
                    alpha_to_coverage_enabled: false, // 4.
                },
                multiview: None,
                cache: None,
            });

        Self { pipeline }
    }

    pub fn pipeline(&self) -> &RenderPipeline {
        &self.pipeline
    }
}
