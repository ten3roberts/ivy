use hecs::World;
use ivy_base::WorldExt;
use ivy_graphics::{
    gizmos::GizmoRenderer, shaders::*, DepthAttachment, FullscreenRenderer, MainCamera,
    MeshRenderer, SkinnedMeshRenderer, WithPass,
};
use ivy_postprocessing::pbr::{create_pbr_pipeline, PBRInfo};
use ivy_rendergraph::{
    AttachmentInfo, CameraNode, CameraNodeInfo, NodeIndex, RenderGraph, SwapchainNode,
};
use ivy_resources::Resources;
use ivy_ui::{Canvas, ImageRenderer, TextRenderer, TextUpdateNode};
use ivy_vulkan::{
    context::SharedVulkanContext,
    vk::{ClearValue, CullModeFlags},
    ImageLayout, ImageUsage, LoadOp, PipelineInfo, StoreOp, Swapchain, Texture, TextureInfo,
};

use crate::{
    Error, GeometryPass, GizmoPass, ImagePass, PostProcessingPass, Result, SkinnedPass, TextPass,
};

/// Create a pbr rendergraph with UI and gizmos
/// This is the most common setup and works well for many use cases.

#[records::record]
/// Preset for common PBR rendering setup with UI and gizmos
pub struct PBRRendering {
    geometry: NodeIndex,
    ui: NodeIndex,
    pbr: NodeIndex,
    gizmo: NodeIndex,
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
    ) -> Result<Self> {
        let camera = world
            .by_tag::<MainCamera>()
            .ok_or(Error::MissingMainCamera)?;

        let canvas = world.by_tag::<Canvas>().ok_or(Error::MissingCanvas)?;

        let context = resources.get_default::<SharedVulkanContext>()?;
        context.wait_idle()?;

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
                usage: ImageUsage::COLOR_ATTACHMENT
                    | ImageUsage::SAMPLED
                    | ImageUsage::TRANSFER_SRC,
                ..Default::default()
            },
        )?)?;

        let pbr_nodes =
            rendergraph.add_nodes(
                create_pbr_pipeline::<GeometryPass, PostProcessingPass, _, _>(
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
                        load_op: LoadOp::DONT_CARE,
                        initial_layout: ImageLayout::UNDEFINED,
                        final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                        resource: final_lit,
                        clear_value: ClearValue::default(),
                    }],
                    &[],
                    pbr_info,
                )?,
            );

        let geometry = pbr_nodes[0];
        let pbr = pbr_nodes[1];

        let gizmo = rendergraph.add_node(CameraNode::<GizmoPass, _>::new(
            context.clone(),
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
                input_attachments: vec![world.get::<DepthAttachment>(camera)?.0],
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

        let ui = rendergraph.add_node(CameraNode::<ImagePass, _>::new(
            context.clone(),
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

        rendergraph.add_node(TextUpdateNode::new(resources, text_renderer)?);

        rendergraph.add_node(SwapchainNode::new(
            context.clone(),
            &resources,
            resources.default()?,
            final_lit,
        )?);

        // Build renderpasses
        rendergraph.build(resources.fetch()?, extent)?;

        resources.insert_default(rendergraph)?;

        Ok(Self {
            geometry,
            ui,
            pbr,
            gizmo,
        })
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

        resources.insert(SkinnedPass(PipelineInfo {
            vs: SKINNED_VERTEX_SHADER,
            fs: DEFAULT_FRAGMENT_SHADER,
            cull_mode: info.cull_mode,
            ..Default::default()
        }))?;

        resources.insert(PostProcessingPass(PipelineInfo {
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
