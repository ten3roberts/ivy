use crate::Result;
use crate::descriptors::DescriptorLayoutInfo;

use super::{descriptors::DescriptorLayoutCache, Error};
use super::{renderpass::*, Extent};
use ash::version::DeviceV1_0;
use ash::Device;
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
