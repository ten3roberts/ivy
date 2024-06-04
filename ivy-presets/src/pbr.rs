use std::sync::Arc;

use flax::World;
use ivy_assets::{Asset, AssetCache};
use ivy_base::{engine, main_camera, Extent, WorldExt};
use ivy_graphics::{
    components::{depth_attachment, swapchain},
    gizmos::GizmoRenderer,
    shaders::*,
    EnvData, GpuCamera, MeshRenderer, SkinnedMeshRenderer,
};
use ivy_postprocessing::pbr::{setup_pbr_nodes, PBRInfo};
use ivy_rendergraph::{
    components::render_graph, AttachmentInfo, CameraNode, CameraNodeInfo, NodeIndex, RenderGraph,
    SwapchainPresentNode,
};
use ivy_ui::{canvas, ImageRenderer, TextRenderer, TextUpdateNode};
use ivy_vulkan::{
    context::VulkanContextService,
    vk::{ClearValue, CullModeFlags},
    ImageLayout, ImageUsage, LoadOp, PipelineInfo, Shader, StoreOp, Texture, TextureInfo,
};
use parking_lot::Mutex;

use crate::{geometry_pass, gizmo_pass, transparent_pass, ui_pass, Error};

pub struct PBRRenderingInfo {
    pub color_usage: ImageUsage,
    pub text_shader: Shader,
    pub ui_shader: Shader,
    pub post_processing_shader: Shader,
    pub gizmo_shader: Shader,
}

// impl Default for PBRRenderingInfo {
//     fn default() -> Self {
//         Self {
//             color_usage: ImageUsage::COLOR_ATTACHMENT
//                 | ImageUsage::SAMPLED
//                 | ImageUsage::TRANSFER_SRC,
//         }
//     }
// }

/// Create a pbr rendergraph with UI and gizmos
/// This is the most common setup and works well for many use cases.
/// Preset for common PBR rendering setup with UI and gizmos
pub struct PBRRendering {
    pub ui: NodeIndex,
    pub gizmo: NodeIndex,
    pub color: Asset<Texture>,
    pub extent: Extent,
    pub render_graph: RenderGraph,
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
    pub fn setup<Env: 'static + Copy + Send + Sync + EnvData>(
        world: &mut World,
        assets: &AssetCache,
        env: Env,
        frames_in_flight: usize,
        info: PBRRenderingInfo,
    ) -> crate::Result<Self> {
        let camera = world
            .by_tag(main_camera())
            .ok_or(Error::MissingMainCamera)?
            .id();

        let context = assets.service::<VulkanContextService>().context();
        context.wait_idle()?;

        GpuCamera::create_gpu_cameras(&context, world, frames_in_flight)?;

        let canvas = world.by_tag(canvas()).ok_or(Error::MissingCanvas)?.id();

        let mut rendergraph = RenderGraph::new(context.clone(), frames_in_flight)?;

        // Setup renderers

        let gizmo_renderer =
            GizmoRenderer::new(context.clone(), (*info.gizmo_shader.pipeline_info).clone())?;
        let mesh_renderer = MeshRenderer::new(context.clone(), 16, frames_in_flight)?;
        let skinned_renderer =
            SkinnedMeshRenderer::new(context.clone(), 16, 128, frames_in_flight)?;

        // let swapchain = resources.get_default::<Swapchain>()?;

        let extent = world.get(engine(), swapchain()).unwrap().extent();

        let final_lit = assets.insert(Texture::new(
            context.clone(),
            &TextureInfo {
                extent,
                mip_levels: 1,
                usage: info.color_usage | ImageUsage::TRANSFER_SRC,
                ..Default::default()
            },
        )?);

        rendergraph.add_nodes(setup_pbr_nodes(
            context.clone(),
            world,
            assets,
            camera,
            (mesh_renderer, skinned_renderer),
            extent,
            frames_in_flight,
            PBRInfo {
                max_lights: 64,
                output: final_lit.clone(),
                read_attachments: vec![],
                post_processing_shader: info.post_processing_shader,
                geometry_pass: geometry_pass(),
                transparent_pass: transparent_pass(),
            },
            env,
        )?);

        let depth_attachment = world.get(camera, depth_attachment()).unwrap().0.clone();

        let gizmo = rendergraph.add_node(CameraNode::new(
            context.clone(),
            world,
            assets,
            camera,
            gizmo_renderer,
            gizmo_pass(),
            CameraNodeInfo {
                name: "Gizmos Node",
                color_attachments: vec![AttachmentInfo {
                    store_op: StoreOp::STORE,
                    load_op: LoadOp::LOAD,
                    initial_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    resource: final_lit.clone(),
                    clear_value: ClearValue::default(),
                }],
                input_attachments: vec![depth_attachment],
                frames_in_flight,
                ..Default::default()
            },
        )?);

        let image_renderer = ImageRenderer::new(context.clone(), 16, frames_in_flight)?;

        let text_renderer = Arc::new(Mutex::new(TextRenderer::new(
            context.clone(),
            16,
            512,
            frames_in_flight,
        )?));

        rendergraph.add_node(TextUpdateNode::new(assets, text_renderer.clone())?);

        let ui = rendergraph.add_node(CameraNode::new(
            context.clone(),
            world,
            assets,
            canvas,
            (image_renderer, text_renderer.clone()),
            ui_pass(),
            CameraNodeInfo {
                name: "UI Node",
                color_attachments: vec![AttachmentInfo {
                    store_op: StoreOp::STORE,
                    load_op: LoadOp::LOAD,
                    initial_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    final_layout: ImageLayout::COLOR_ATTACHMENT_OPTIMAL,
                    resource: final_lit.clone(),
                    clear_value: ClearValue::default(),
                }],
                buffer_reads: vec![text_renderer.lock().vertex_buffer()],
                frames_in_flight,
                ..Default::default()
            },
        )?);

        // Build renderpasses
        rendergraph.build(extent)?;

        Ok(Self {
            color: final_lit,
            ui,
            gizmo,
            extent,
            render_graph: rendergraph,
        })
    }

    pub fn using_swapchain(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
    ) -> crate::Result<&Self> {
        let context = assets.service::<VulkanContextService>().context();

        self.render_graph.add_node(SwapchainPresentNode::new(
            world,
            context.clone(),
            assets,
            self.color.clone(),
        )?);

        self.render_graph.build(self.extent)?;
        Ok(self)
    }

    pub fn install(self, world: &mut World) {
        world
            .set(
                engine(),
                render_graph(),
                Arc::new(Mutex::new(self.render_graph)),
            )
            .unwrap();
    }

    pub fn color(&self) -> &Asset<Texture> {
        &self.color
    }

    pub fn extent(&self) -> Extent {
        self.extent
    }
}

pub fn default_geometry_shader(cull_mode: CullModeFlags) -> PipelineInfo {
    PipelineInfo {
        vs: DEFAULT_VERTEX_SHADER,
        fs: DEFAULT_FRAGMENT_SHADER,
        cull_mode,
        ..Default::default()
    }
}

pub fn default_transparent_shader() -> PipelineInfo {
    PipelineInfo {
        vs: GLASS_VERTEX_SHADER,
        fs: GLASS_FRAGMENT_SHADER,
        cull_mode: CullModeFlags::BACK,
        ..Default::default()
    }
}

pub fn default_skinned_shader() -> PipelineInfo {
    PipelineInfo {
        vs: SKINNED_VERTEX_SHADER,
        fs: DEFAULT_FRAGMENT_SHADER,
        cull_mode: CullModeFlags::BACK,
        ..Default::default()
    }
}

pub fn default_post_processing_shader() -> PipelineInfo {
    PipelineInfo {
        vs: FULLSCREEN_SHADER,
        fs: POSTPROCESSING_SHADER,
        cull_mode: CullModeFlags::NONE,
        ..Default::default()
    }
}

pub fn default_image_shader() -> PipelineInfo {
    PipelineInfo {
        vs: IMAGE_VERTEX_SHADER,
        fs: IMAGE_FRAGMENT_SHADER,
        blending: true,
        cull_mode: CullModeFlags::BACK,
        ..Default::default()
    }
}

pub fn default_text_shader() -> PipelineInfo {
    PipelineInfo {
        vs: TEXT_VERTEX_SHADER,
        fs: TEXT_FRAGMENT_SHADER,
        cull_mode: CullModeFlags::BACK,
        blending: true,
        ..Default::default()
    }
}

pub fn default_gizmo_shader() -> PipelineInfo {
    PipelineInfo {
        vs: GIZMO_VERTEX_SHADER,
        fs: GIZMO_FRAGMENT_SHADER,
        cull_mode: CullModeFlags::NONE,
        ..Default::default()
    }
}

pub struct PipelinesInfo {
    /// The cull mode to use where it makes sense
    cull_mode: CullModeFlags,
}

impl PipelinesInfo {
    pub fn new(cull_mode: CullModeFlags) -> Self {
        Self { cull_mode }
    }

    pub fn cull_mode(&self) -> CullModeFlags {
        self.cull_mode
    }
}

impl Default for PipelinesInfo {
    fn default() -> Self {
        Self {
            cull_mode: CullModeFlags::BACK,
        }
    }
}
