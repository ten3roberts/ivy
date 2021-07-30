use crate::{
    descriptors::{DescriptorLayoutCache, DescriptorLayoutInfo},
    renderpass::*,
    Error, Extent, Result,
};
use arrayvec::ArrayVec;
use ash::{version::DeviceV1_0, vk::PipelineLayout};
use ash::{vk::PushConstantRange, Device};
use std::{ffi::CString, sync::Arc};
use std::{fs::File, path::PathBuf};

use ash::vk;

mod shader;
use shader::*;

pub struct PipelineInfo<'a> {
    pub vertexshader: PathBuf,
    pub fragmentshader: PathBuf,
    pub vertex_binding: vk::VertexInputBindingDescription,
    pub vertex_attributes: &'static [vk::VertexInputAttributeDescription],
    pub samples: vk::SampleCountFlags,
    pub extent: Extent,
    pub subpass: u32,
    pub polygon_mode: vk::PolygonMode,
    pub cull_mode: vk::CullModeFlags,
    pub front_face: vk::FrontFace,
    /// The bindings specified
    pub set_layouts: &'a [DescriptorLayoutInfo],
}

impl<'a> Default for PipelineInfo<'a> {
    fn default() -> Self {
        Self {
            vertexshader: "".into(),
            fragmentshader: "".into(),
            vertex_binding: vk::VertexInputBindingDescription::default(),
            vertex_attributes: &[],
            samples: vk::SampleCountFlags::TYPE_1,
            extent: (0, 0).into(),
            subpass: 0,
            polygon_mode: vk::PolygonMode::FILL,
            cull_mode: vk::CullModeFlags::BACK,
            front_face: vk::FrontFace::COUNTER_CLOCKWISE,
            set_layouts: &[],
        }
    }
}

pub struct Pipeline {
    device: Arc<Device>,
    pipeline: vk::Pipeline,
    layout: vk::PipelineLayout,
}

impl Pipeline {
    pub fn new(
        device: Arc<Device>,
        layout_cache: &mut DescriptorLayoutCache,
        renderpass: &RenderPass,
        info: PipelineInfo,
    ) -> Result<Self> {
        let mut vertexshader = File::open(&info.vertexshader)
            .map_err(|e| Error::Io(e, Some(info.vertexshader.clone())))?;

        let mut fragmentshader = File::open(&info.fragmentshader)
            .map_err(|e| Error::Io(e, Some(info.fragmentshader.clone())))?;

        let vertexshader = ShaderModule::new(&device, &mut vertexshader)?;
        let fragmentshader = ShaderModule::new(&device, &mut fragmentshader)?;

        let layout = shader::reflect(
            &device,
            &[&vertexshader, &fragmentshader],
            layout_cache,
            info.set_layouts,
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

        let vertex_binding_descriptions = [info.vertex_binding];

        // No vertices for now
        let vertex_input_info = vk::PipelineVertexInputStateCreateInfo::builder()
            .vertex_binding_descriptions(&vertex_binding_descriptions)
            .vertex_attribute_descriptions(&info.vertex_attributes);

        let input_assembly = vk::PipelineInputAssemblyStateCreateInfo::builder()
            .topology(vk::PrimitiveTopology::TRIANGLE_LIST)
            .primitive_restart_enable(false);

        let viewports = [vk::Viewport {
            x: 0.0f32,
            y: 0.0f32,
            width: info.extent.width as _,
            height: info.extent.height as _,
            min_depth: 0.0f32,
            max_depth: 1.0f32,
        }];

        let scissors = [vk::Rect2D {
            offset: vk::Offset2D { x: 0, y: 0 },
            extent: info.extent.into(),
        }];

        let viewport_state = vk::PipelineViewportStateCreateInfo::builder()
            .viewports(&viewports)
            .scissors(&scissors);

        let rasterizer = vk::PipelineRasterizationStateCreateInfo::builder()
            // Clamp pixels outside far and near
            .depth_clamp_enable(false)
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

        let color_blend_attachments = [vk::PipelineColorBlendAttachmentState::builder()
            .color_write_mask(
                vk::ColorComponentFlags::R
                    | vk::ColorComponentFlags::G
                    | vk::ColorComponentFlags::B
                    | vk::ColorComponentFlags::A,
            )
            .blend_enable(false)
            .src_color_blend_factor(vk::BlendFactor::ONE)
            .dst_color_blend_factor(vk::BlendFactor::ZERO)
            .color_blend_op(vk::BlendOp::ADD)
            .src_alpha_blend_factor(vk::BlendFactor::ONE)
            .dst_alpha_blend_factor(vk::BlendFactor::ZERO)
            .alpha_blend_op(vk::BlendOp::ADD)
            .build()];

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
            .depth_stencil_state(&depth_stencil)
            .layout(layout)
            .render_pass(renderpass.renderpass())
            .subpass(info.subpass)
            .build();

        let pipeline = unsafe {
            device
                .create_graphics_pipelines(vk::PipelineCache::null(), &[create_info], None)
                .map_err(|(_, e)| e)?
        }[0];

        // Destroy shader modules
        vertexshader.destroy(&device);
        fragmentshader.destroy(&device);

        Ok(Pipeline {
            device,
            pipeline,
            layout,
        })
    }

    // Creates a raw pipeline layout
    pub fn create_layout(
        device: &Device,
        sets: &[DescriptorLayoutInfo],
        push_constant_ranges: &[PushConstantRange],
        layout_cache: &mut DescriptorLayoutCache,
    ) -> Result<PipelineLayout> {
        let set_layouts = sets
            .iter()
            .take_while(|set| !set.bindings().is_empty())
            .map(|set| layout_cache.get(set))
            .collect::<std::result::Result<ArrayVec<[_; MAX_SETS]>, _>>()?;

        let create_info = vk::PipelineLayoutCreateInfo {
            set_layout_count: set_layouts.len() as u32,
            p_set_layouts: set_layouts.as_ptr(),
            push_constant_range_count: push_constant_ranges.len() as u32,
            p_push_constant_ranges: push_constant_ranges.as_ptr(),
            ..Default::default()
        };

        let pipeline_layout = unsafe { device.create_pipeline_layout(&create_info, None)? };
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
        unsafe { self.device.destroy_pipeline(self.pipeline, None) }
        unsafe { self.device.destroy_pipeline_layout(self.layout, None) }
    }
}
