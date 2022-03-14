use crate::context::SharedVulkanContext;
use crate::traits::FromExtent;
use crate::{descriptors::DescriptorLayoutInfo, Result, VertexDesc, VulkanContext};
use ash::vk::{
    BlendFactor, BlendOp, ColorComponentFlags, Extent2D, PipelineColorBlendAttachmentState,
    PipelineLayout, PrimitiveTopology, PushConstantRange,
};
use ivy_base::Extent;
use smallvec::SmallVec;
use std::borrow::Cow;
use std::ffi::CString;

use ash::vk;

mod shader;
pub use shader::ShaderModuleInfo;
use shader::*;

#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PipelineInfo {
    pub vs: ShaderModuleInfo,
    pub fs: ShaderModuleInfo,
    // Enable alpha blending,
    pub blending: bool,
    pub depth_clamp: bool,
    pub color_blend_op: BlendOp,
    pub alpha_blend_op: BlendOp,
    pub src_color: BlendFactor,
    pub dst_color: BlendFactor,
    pub dst_alpha: BlendFactor,
    pub src_alpha: BlendFactor,
    pub topology: PrimitiveTopology,
    pub samples: vk::SampleCountFlags,
    pub polygon_mode: vk::PolygonMode,
    pub cull_mode: vk::CullModeFlags,
    pub front_face: vk::FrontFace,
    /// The bindings specified
    pub set_layouts: Cow<'static, [DescriptorLayoutInfo]>,
}

impl Default for PipelineInfo {
    fn default() -> Self {
        Self {
            depth_clamp: false,
            blending: false,
            vs: "".into(),
            fs: "".into(),
            samples: vk::SampleCountFlags::TYPE_1,
            polygon_mode: vk::PolygonMode::FILL,
            cull_mode: vk::CullModeFlags::BACK,
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            set_layouts: Cow::Borrowed(&[]),
            topology: PrimitiveTopology::TRIANGLE_LIST,
            color_blend_op: BlendOp::ADD,
            alpha_blend_op: BlendOp::ADD,
            src_color: BlendFactor::SRC_ALPHA,
            dst_color: BlendFactor::ONE_MINUS_SRC_ALPHA,
            dst_alpha: BlendFactor::ONE,
            src_alpha: BlendFactor::ONE,
        }
    }
}

pub struct Pipeline {
    context: SharedVulkanContext,
    pipeline: vk::Pipeline,
    layout: vk::PipelineLayout,
}

impl Pipeline {
    pub fn new<V>(
        context: SharedVulkanContext,
        info: &PipelineInfo,
        pass_info: &PassInfo,
    ) -> Result<Self>
    where
        V: VertexDesc,
    {
        let device = context.device();

        let vertexshader = ShaderModule::new(device, &info.vs)?;
        let fragmentshader = ShaderModule::new(device, &info.fs)?;

        let layout = shader::reflect(
            &context,
            &[&vertexshader, &fragmentshader],
            info.set_layouts.as_ref(),
        )?;

        let entrypoint = CString::new("main").unwrap();

        let shader_stages = [
            vk::PipelineShaderStageCreateInfo::builder()
                .module(vertexshader.module)
                .stage(vk::ShaderStageFlags::VERTEX)
                .name(&entrypoint)
                .build(),
            vk::PipelineShaderStageCreateInfo::builder()
                .module(fragmentshader.module)
                .stage(vk::ShaderStageFlags::FRAGMENT)
                .name(&entrypoint)
                .build(),
        ];

        // No vertices for now
        let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(V::BINDING_DESCRIPTIONS)
            .vertex_attribute_descriptions(V::ATTRIBUTE_DESCRIPTIONS);

        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::builder()
            .topology(info.topology)
            .primitive_restart_enable(false);

        let viewports = [vk::Viewport {
            x: 0.0f32,
            y: 0.0f32,
            width: pass_info.extent.width as _,
            height: pass_info.extent.height as _,
            min_depth: 0.0f32,
            max_depth: 1.0f32,
        }];

        let scissors = [vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: Extent2D::from_extent(pass_info.extent),
        }];

        let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
            .viewports(&viewports)
            .scissors(&scissors);

        let rasterizer = vk::PipelineRasterizationStateCreateInfo::builder()
            // Clamp pixels outside far and near
            .depth_clamp_enable(info.depth_clamp)
            // If true: Discard all pixels
            .rasterizer_discard_enable(false)
            .polygon_mode(info.polygon_mode)
            .line_width(1.0)
            .cull_mode(info.cull_mode)
            .front_face(info.front_face)
            .depth_bias_enable(false)
            .depth_bias_constant_factor(0.0)
            .depth_bias_clamp(0.0)
            .depth_bias_slope_factor(0.0);

        let multisampling = vk::PipelineMultisampleStateCreateInfo::builder()
            .sample_shading_enable(false)
            .rasterization_samples(info.samples)
            .min_sample_shading(1.0)
            .alpha_to_coverage_enable(false)
            .alpha_to_one_enable(false);

        let color_blend_attachments = (0..pass_info.color_attachment_count)
            .map(|_| {
                if info.blending {
                    PipelineColorBlendAttachmentState {
                        blend_enable: vk::TRUE,
                        src_color_blend_factor: info.src_color,
                        dst_color_blend_factor: info.dst_color,
                        color_blend_op: info.color_blend_op,
                        src_alpha_blend_factor: info.src_alpha,
                        dst_alpha_blend_factor: info.dst_alpha,
                        alpha_blend_op: BlendOp::ADD,
                        color_write_mask: ColorComponentFlags::R
                            | ColorComponentFlags::G
                            | ColorComponentFlags::B
                            | ColorComponentFlags::A,
                    }
                } else {
                    PipelineColorBlendAttachmentState {
                        blend_enable: vk::FALSE,
                        src_color_blend_factor: BlendFactor::ONE,
                        dst_color_blend_factor: BlendFactor::ZERO,
                        color_blend_op: BlendOp::ADD,
                        src_alpha_blend_factor: BlendFactor::ONE,
                        dst_alpha_blend_factor: BlendFactor::ZERO,
                        alpha_blend_op: BlendOp::ADD,
                        color_write_mask: ColorComponentFlags::R
                            | ColorComponentFlags::G
                            | ColorComponentFlags::B
                            | ColorComponentFlags::A,
                    }
                }
            })
            .collect::<Vec<_>>();

        let color_blending = vk::PipelineColorBlendStateCreateInfo::builder()
            .logic_op_enable(false)
            .attachments(&color_blend_attachments)
            .logic_op(vk::LogicOp::COPY);

        let depth_stencil = vk::PipelineDepthStencilStateCreateInfo {
            s_type: vk::StructureType::PIPELINE_DEPTH_STENCIL_STATE_CREATE_INFO,
            depth_test_enable: vk::TRUE,
            depth_write_enable: vk::TRUE,
            depth_compare_op: vk::CompareOp::LESS,
            depth_bounds_test_enable: vk::FALSE,
            stencil_test_enable: vk::FALSE,
            min_depth_bounds: 0.0,
            max_depth_bounds: 1.0,
            ..Default::default()
        };

        let create_info = vk::GraphicsPipelineCreateInfo::builder()
            .stages(&shader_stages)
            .vertex_input_state(&vertex_input_info)
            .input_assembly_state(&input_assembly)
            .viewport_state(&viewport_state)
            .rasterization_state(&rasterizer)
            .multisample_state(&multisampling)
            .color_blend_state(&color_blending)
            .layout(layout)
            .render_pass(pass_info.renderpass)
            .subpass(pass_info.subpass);

        let create_info = if pass_info.depth_attachment {
            create_info.depth_stencil_state(&depth_stencil)
        } else {
            create_info
        };

        let create_info = create_info.build();

        let pipeline = unsafe {
            device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[create_info], None)
                .map_err(|(_, e)| e)?
        }[0];

        // Destroy shader modules
        vertexshader.destroy(device);
        fragmentshader.destroy(device);

        Ok(Pipeline {
            context,
            pipeline,
            layout,
        })
    }

    // Creates a raw pipeline layout
    pub fn create_layout(
        context: &VulkanContext,
        sets: &[DescriptorLayoutInfo],
        push_constant_ranges: &[PushConstantRange],
    ) -> Result<PipelineLayout> {
        let layout_cache = context.layout_cache();

        let set_layouts = sets
            .iter()
            .take_while(|set| !set.bindings().is_empty())
            .map(|set| layout_cache.get(set))
            .collect::<std::result::Result<SmallVec<[_; MAX_SETS]>, _>>()?;

        let create_info = vk::PipelineLayoutCreateInfo {
            set_layout_count: set_layouts.len() as u32,
            p_set_layouts: set_layouts.as_ptr(),
            push_constant_range_count: push_constant_ranges.len() as u32,
            p_push_constant_ranges: push_constant_ranges.as_ptr(),
            ..Default::default()
        };

        let pipeline_layout = unsafe {
            context
                .device()
                .create_pipeline_layout(&create_info, None)?
        };
        Ok(pipeline_layout)
    }

    /// Returns the raw vulkan pipeline handle.
    pub fn pipeline(&self) -> vk::Pipeline {
        self.pipeline
    }

    // Returns the pipeline layout.
    pub fn layout(&self) -> vk::PipelineLayout {
        self.layout
    }
}

impl AsRef<vk::Pipeline> for Pipeline {
    fn as_ref(&self) -> &vk::Pipeline {
        &self.pipeline
    }
}

impl From<&Pipeline> for vk::Pipeline {
    fn from(val: &Pipeline) -> Self {
        val.pipeline
    }
}

impl AsRef<vk::PipelineLayout> for Pipeline {
    fn as_ref(&self) -> &vk::PipelineLayout {
        &self.layout
    }
}

impl From<&Pipeline> for vk::PipelineLayout {
    fn from(val: &Pipeline) -> Self {
        val.layout
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        unsafe { self.context.device().destroy_pipeline(self.pipeline, None) }
        unsafe {
            self.context
                .device()
                .destroy_pipeline_layout(self.layout, None)
        }
    }
}

#[derive(Default)]
#[records::record]
pub struct PassInfo {
    renderpass: vk::RenderPass,
    subpass: u32,
    extent: Extent,
    color_attachment_count: u32,
    depth_attachment: bool,
}
