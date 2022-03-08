use hecs::World;
use ivy_base::{Extent, WorldExt};
use ivy_graphics::{
    gizmos::GizmoRenderer, shaders::*, DepthAttachment, FullscreenRenderer, GpuCameraData,
    MainCamera, MeshRenderer, SkinnedMeshRenderer, WithPass,
};
use ivy_postprocessing::pbr::{create_pbr_pipeline, PBRInfo};
use ivy_rendergraph::{
    AttachmentInfo, CameraNode, CameraNodeInfo, NodeIndex, RenderGraph, SwapchainNode, TransferNode,
};
use ivy_resources::{Handle, Resources};
use ivy_ui::{Canvas, ImageRenderer, TextRenderer, TextUpdateNode};
use ivy_vulkan::{
    context::SharedVulkanContext,
    vk::{ClearValue, CullModeFlags, ImageAspectFlags, ShaderStageFlags},
    AddressMode, ClearValueExt, Format, ImageLayout, ImageUsage, LoadOp, PipelineInfo, Sampler,
    SamplerInfo, StoreOp, Swapchain, Texture, TextureInfo,
};

use crate::{
    Error, GeometryPass, GizmoPass, ImagePass, PbrPass, Result, SkinnedPass, TextPass,
    TransparentPass,
};

/// Create a pbr rendergraph with UI and gizmos
/// This is the most common setup and works well for many use cases.

#[records::record]
/// Preset for common PBR rendering setup with UI and gizmos
pub struct PBRRendering {
    ui: NodeIndex,
    gizmo: NodeIndex,
    color: Handle<Texture>,
    extent: Extent,
}

#[records::record]
pub struct PBRRenderingInfo {
    pub color_usage: ImageUsage,
}

impl Default for PBRRenderingInfo {
    fn default() -> Self {
        Self {
            color_usage: ImageUsage::COLOR_ATTACHMENT
                | ImageUsage::SAMPLED
                | ImageUsage::TRANSFER_SRC,
        }
    }
}

impl PBRRendering {
    /// Setup a PBR rendergraph consisting of
    /// - Geometry rendering
    /// - Deferred PBR shading
    /// - UI image and text rendering
    /// - Composition and swapchain presentation
    ///
    /// Requires the existence of [ Swapchain ](ivy_vulkan::Swapchain), [Canvas](ivy_ui::Canvas),
    /// [MainCamera](ivy_graphics::MainCamera).
    pub fn setup<Env: 'static + Copy + Send + Sync>(
        world: &mut World,
        resources: &Resources,
        pbr_info: PBRInfo<Env>,
        frames_in_flight: usize,
        info: PBRRenderingInfo,
    ) -> Result<Self> {
        let camera = world
            .by_tag::<MainCamera>()
            .ok_or(Error::MissingMainCamera)?;

        let context = resources.get_default::<SharedVulkanContext>()?;
        context.wait_idle()?;

        GpuCameraData::create_gpu_cameras(&context, world, frames_in_flight)?;

        let canvas = world.by_tag::<Canvas>().ok_or(Error::MissingCanvas)?;

        let mut rendergraph = RenderGraph::new(context.clone(), frames_in_flight)?;

        // Setup renderers
        resources
            .default_entry()?
            .or_insert_with(|| FullscreenRenderer::new());

        let gizmo_renderer = resources
            .default_entry()?
            .or_try_insert_with(|| GizmoRenderer::new(context.clone()))?
            .handle;

        let mesh_renderer = resources
            .default_entry()?
            .or_try_insert_with(|| MeshRenderer::new(context.clone(), 16, frames_in_flight))?
            .handle;

        let skinned_renderer = resources
            .default_entry()?
            .or_try_insert_with(|| {
                SkinnedMeshRenderer::new(context.clone(), 16, 128, frames_in_flight)
            })?
            .handle;

        let swapchain = resources.get_default::<Swapchain>()?;

        let extent = swapchain.extent();

        let final_lit = resources.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent,
                mip_levels: 1,
                usage: info.color_usage | ImageUsage::TRANSFER_SRC,
                ..Default::default()
            },
        )?)?;

        let screenview = resources.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent,
                mip_levels: 1,
                usage: ImageUsage::TRANSFER_DST | ImageUsage::SAMPLED,
                ..Default::default()
            },
        )?)?;

        let screenview_d = resources.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent,
                mip_levels: 1,
                usage: ImageUsage::TRANSFER_DST
                    | ImageUsage::SAMPLED
                    | ImageUsage::DEPTH_STENCIL_ATTACHMENT,

                format: Format::D32_SFLOAT,
                ..Default::default()
            },
        )?)?;

        let _pbr_nodes = rendergraph.add_nodes(create_pbr_pipeline::<GeometryPass, PbrPass, _, _>(
            context.clone(),
            world,
            &resources,
            camera,
            (
                mesh_renderer,
                WithPass::<SkinnedPass, _>::new(skinned_renderer),
            ),
            extent,
            frames_in_flight,
            &[],
            &[AttachmentInfo {
                store_op: StoreOp::STORE,
                load_op: LoadOp::CLEAR,
                initial_layout: ImageLayout::UNDEFINED,
                final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                resource: final_lit,
                clear_value: ClearValue::color(0.0, 0.0, 0.0, 1.0),
            }],
            &[],
            pbr_info,
        )?);

        let depth_attachment = **world.get::<DepthAttachment>(camera)?;

        let gizmo = rendergraph.add_node(CameraNode::<GizmoPass, _>::new(
            context.clone(),
            world,
            resources,
            camera,
            gizmo_renderer,
            CameraNodeInfo {
                name: "Gizmos Node",
                color_attachments: vec![AttachmentInfo {
                    store_op: StoreOp::STORE,
                    load_op: LoadOp::LOAD,
                    initial_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    resource: final_lit,
                    clear_value: ClearValue::default(),
                }],
                input_attachments: vec![depth_attachment],
                frames_in_flight,
                ..Default::default()
            },
        )?);

        let sampler: Handle<Sampler> = resources.load(SamplerInfo {
            address_mode: AddressMode::CLAMP_TO_BORDER,
            ..Default::default()
        })??;

        // Copy lit to the attachment to read
        rendergraph.add_node(TransferNode::new(
            final_lit,
            screenview,
            ImageLayout::TRANSFER_SRC_OPTIMAL,
            ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
            ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            ImageAspectFlags::COLOR,
        )?);

        // Copy lit to the attachment to read
        rendergraph.add_node(TransferNode::new(
            **world.get::<DepthAttachment>(camera)?,
            screenview_d,
            ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
            ImageLayout::SHADER_READ_ONLY_OPTIMAL,
            ImageAspectFlags::DEPTH,
        )?);

        rendergraph.add_node(CameraNode::<TransparentPass, _>::new(
            context.clone(),
            world,
            resources,
            camera,
            mesh_renderer,
            CameraNodeInfo {
                name: "Transparent",
                color_attachments: vec![AttachmentInfo {
                    store_op: StoreOp::STORE,
                    load_op: LoadOp::LOAD,
                    initial_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    resource: final_lit,
                    clear_value: ClearValue::color(0.0, 0.0, 0.0, 0.0),
                }],
                depth_attachment: Some(AttachmentInfo {
                    resource: depth_attachment,
                    store_op: StoreOp::STORE,
                    load_op: LoadOp::LOAD,
                    initial_layout: ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                    final_layout: ImageLayout::DEPTH_STENCIL_ATTACHMENT_OPTIMAL,
                    ..Default::default()
                }),
                read_attachments: &[(screenview, sampler), (screenview_d, sampler)],
                additional: vec![final_lit],
                camera_stage: ShaderStageFlags::VERTEX | ShaderStageFlags::FRAGMENT,
                frames_in_flight,
                ..Default::default()
            },
        )?);

        let image_renderer = resources
            .default_entry()?
            .or_try_insert_with(|| ImageRenderer::new(context.clone(), 16, frames_in_flight))?
            .handle;

        let text_renderer = resources
            .default_entry()?
            .or_try_insert_with(|| TextRenderer::new(context.clone(), 16, 512, frames_in_flight))?
            .handle;

        rendergraph.add_node(TextUpdateNode::new(resources, text_renderer)?);

        let ui = rendergraph.add_node(CameraNode::<ImagePass, _>::new(
            context.clone(),
            world,
            resources,
            canvas,
            (image_renderer, WithPass::<TextPass, _>::new(text_renderer)),
            CameraNodeInfo {
                name: "UI Node",
                color_attachments: vec![AttachmentInfo {
                    store_op: StoreOp::STORE,
                    load_op: LoadOp::LOAD,
                    initial_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    resource: final_lit,
                    clear_value: ClearValue::default(),
                }],
                buffer_reads: vec![resources.get(text_renderer)?.vertex_buffer()],
                frames_in_flight,
                ..Default::default()
            },
        )?);

        // Build renderpasses
        rendergraph.build(resources.fetch()?, extent)?;

        resources.insert_default(rendergraph)?;

        Ok(Self {
            color: final_lit,
            ui,
            gizmo,
            extent,
        })
    }

    pub fn using_swapchain(&self, resources: &Resources) -> Result<&Self> {
        let mut rendergraph = resources.get_default_mut::<RenderGraph>()?;
        let context = resources.get_default::<SharedVulkanContext>()?;

        rendergraph.add_node(SwapchainNode::new(
            context.clone(),
            &resources,
            resources.default()?,
            self.color,
        )?);

        rendergraph.build(resources.fetch()?, self.extent)?;
        Ok(self)
    }

    /// Setups basic pipelines and inserts them into the resource store
    pub fn setup_pipelines(&self, resources: &Resources, info: PipelinesInfo) -> Result<()> {
        // Create pipelines
        resources.insert(GeometryPass(PipelineInfo {
            vs: DEFAULT_VERTEX_SHADER,
            fs: DEFAULT_FRAGMENT_SHADER,
            cull_mode: info.cull_mode,
            ..Default::default()
        }))?;

        resources.insert(TransparentPass(PipelineInfo {
            vs: DEFAULT_VERTEX_SHADER,
            fs: GLASS_FRAGMENT_SHADER,
            cull_mode: info.cull_mode,
            ..Default::default()
        }))?;
        resources.insert(SkinnedPass(PipelineInfo {
            vs: SKINNED_VERTEX_SHADER,
            fs: DEFAULT_FRAGMENT_SHADER,
            cull_mode: info.cull_mode,
            ..Default::default()
        }))?;

        resources.insert(PbrPass(PipelineInfo {
            vs: FULLSCREEN_SHADER,
            fs: PBR_SHADER,
            cull_mode: CullModeFlags::NONE,
            ..Default::default()
        }))?;

        resources.insert(ImagePass(PipelineInfo {
            vs: IMAGE_VERTEX_SHADER,
            fs: IMAGE_FRAGMENT_SHADER,
            blending: true,
            cull_mode: CullModeFlags::BACK,
            ..Default::default()
        }))?;

        resources.insert(TextPass(PipelineInfo {
            vs: TEXT_VERTEX_SHADER,
            fs: TEXT_FRAGMENT_SHADER,
            cull_mode: CullModeFlags::BACK,
            blending: true,
            ..Default::default()
        }))?;

        resources.insert(GizmoPass(PipelineInfo {
            vs: GIZMO_VERTEX_SHADER,
            fs: GIZMO_FRAGMENT_SHADER,
            cull_mode: CullModeFlags::NONE,
            blending: true,
            ..Default::default()
        }))?;

        Ok(())
    }

    pub fn color(&self) -> Handle<Texture> {
        self.color
    }

    pub fn extent(&self) -> Extent {
        self.extent
    }
}

#[records::record]
pub struct PipelinesInfo {
    /// The cull mode to use where it makes sense
    cull_mode: CullModeFlags,
}

impl Default for PipelinesInfo {
    fn default() -> Self {
        Self {
            cull_mode: CullModeFlags::BACK,
        }
    }
}
