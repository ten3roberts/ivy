use std::{future::ready, mem::size_of};

use bytemuck::{Pod, Zeroable};
use flax::World;
use futures::{stream, StreamExt};
use glam::Vec2;
use image::DynamicImage;
use ivy_assets::{
    stored::{DynamicStore, Handle},
    AssetCache, AsyncAssetExt,
};
use ivy_core::components::engine;
use ivy_ui::{components::ui_instance, node::UiRenderNode, violet::wgpu::app::AppInstance};
use ivy_wgpu::{
    components::{forward_pass, transparent_pass},
    renderer::{
        gizmos_renderer::GizmosRendererNode,
        mesh_renderer::MeshRenderer,
        shadowmapping::{LightShadowCamera, ShadowMapNode},
        CameraNode, LightManager, MsaaResolve, ObjectManager, SkyboxTextures,
    },
    rendergraph::{BufferDesc, ManagedTextureDesc, Node, RenderGraph, TextureHandle},
    types::{texture::max_mip_levels, PhysicalSize, TypedBuffer},
    Gpu,
};
use wgpu::{BufferUsages, Extent3d, TextureDimension, TextureFormat};

use crate::{
    bloom::BloomNode,
    depth_resolve::MsaaDepthResolve,
    dof::DepthOfFieldNode,
    effects::{
        BloomConfig, ColorGradingConfig, DofConfig, MsaaConfig, PostProcessingEffect,
        ShadowMapConfig, SkyboxConfig,
    },
    hdri::{HdriProcessor, HdriProcessorNode},
    skybox::SkyboxRenderer,
    tonemap::TonemapNode,
};

/// Dynamic color grading controller for smooth transitions
#[derive(Clone)]
pub enum EasingFunction {
    Linear,
    SmoothStep,
    Exponential,
}

pub struct ColorGradingController {
    current: ColorGradingConfig,
    target: ColorGradingConfig,
    transition_duration: f32,
    elapsed_time: f32,
    easing_function: EasingFunction,
}

impl ColorGradingController {
    pub fn new(initial_config: ColorGradingConfig) -> Self {
        Self {
            current: initial_config.clone(),
            target: initial_config,
            transition_duration: 0.0,
            elapsed_time: 0.0,
            easing_function: EasingFunction::SmoothStep,
        }
    }

    pub fn transition_to(&mut self, target: ColorGradingConfig, duration: f32) {
        self.target = target;
        self.transition_duration = duration;
        self.elapsed_time = 0.0;
    }

    pub fn update(&mut self, delta_time: f32) -> &ColorGradingConfig {
        if self.elapsed_time < self.transition_duration {
            self.elapsed_time += delta_time;
            let t = (self.elapsed_time / self.transition_duration).min(1.0);
            let eased_t = self.apply_easing(t);

            self.current = self.interpolate_configs(&self.current, &self.target, eased_t);
        }
        &self.current
    }

    fn interpolate_configs(
        &self,
        a: &ColorGradingConfig,
        b: &ColorGradingConfig,
        t: f32,
    ) -> ColorGradingConfig {
        ColorGradingConfig {
            exposure: a.exposure + (b.exposure - a.exposure) * t,
            contrast: a.contrast + (b.contrast - a.contrast) * t,
            saturation: a.saturation + (b.saturation - a.saturation) * t,
            lift: a.lift.lerp(b.lift, t),
            gamma: a.gamma.lerp(b.gamma, t),
            gain: a.gain.lerp(b.gain, t),
            temperature: a.temperature + (b.temperature - a.temperature) * t,
            tint: a.tint + (b.tint - a.tint) * t,
            _padding: Default::default(),
        }
    }

    fn apply_easing(&self, t: f32) -> f32 {
        match self.easing_function {
            EasingFunction::Linear => t,
            EasingFunction::SmoothStep => t * t * (3.0 - 2.0 * t),
            EasingFunction::Exponential => 1.0 - (-t * 5.0).exp(),
        }
    }
}

/// Pre-configured render graph suited for PBR render pipelines
pub struct PbrRenderGraphConfig {
    pub shadow_map_config: Option<ShadowMapConfig>,
    pub msaa: Option<MsaaConfig>,
    pub post_processing_effects: Vec<Box<dyn PostProcessingEffect>>,
    pub color_grading: ColorGradingConfig,
    pub skybox: Option<SkyboxConfig>,
    pub hdr_format: Option<TextureFormat>,
    pub label: String,
}

impl Default for PbrRenderGraphConfig {
    fn default() -> Self {
        Self {
            shadow_map_config: Some(Default::default()),
            msaa: Some(Default::default()),
            post_processing_effects: vec![],
            color_grading: ColorGradingConfig::cinematic(),
            skybox: None,
            hdr_format: Some(TextureFormat::Rgba16Float),
            label: "pbr".into(),
        }
    }
}

pub struct PbrRenderGraphTextures {
    screensized: Vec<TextureHandle>,
}

impl PbrRenderGraphTextures {
    pub fn screensized(&self) -> &[TextureHandle] {
        &self.screensized
    }
}

impl PbrRenderGraphConfig {
    #[allow(clippy::too_many_arguments)]
    // TODO: fix arguments count
    pub fn configure(
        self,
        world: &mut World,
        gpu: &Gpu,
        assets: &AssetCache,
        store: &mut DynamicStore,
        render_graph: &mut RenderGraph,
        destination: TextureHandle,
    ) -> PbrRenderGraphTextures {
        let object_manager = store.insert(ObjectManager::new(world, gpu));

        let extent = Extent3d {
            width: 0,
            height: 0,
            depth_or_array_layers: 1,
        };

        let target_format = self.hdr_format.unwrap_or(TextureFormat::Rgba8UnormSrgb);

        // TODO: extend with generic effects
        let needs_indirection_target =
            self.hdr_format.is_some() || !self.post_processing_effects.is_empty();

        tracing::info!(?target_format);
        let final_color = if needs_indirection_target {
            render_graph.resources.insert_texture(ManagedTextureDesc {
                label: format!("{}.final_color", self.label).into(),
                extent,
                dimension: wgpu::TextureDimension::D2,
                format: target_format,
                mip_level_count: 1,
                sample_count: 1,
                persistent: false,
            })
        } else {
            destination
        };

        let sample_count = self.msaa.as_ref().map(|v| v.sample_count).unwrap_or(1);

        let depth_texture = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "depth_texture".into(),
            extent,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            mip_level_count: 1,
            sample_count,
            persistent: false,
        });

        // separate depth textures to render gizmos on top
        let gizmos_depth_texture = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "gizmos_depth_texture".into(),
            extent,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            mip_level_count: 1,
            sample_count: 1,
            persistent: false,
        });

        let resolved_depth_texture = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "resolved_depth_texture".into(),
            extent,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba16Float,
            mip_level_count: 1,
            sample_count: 1,
            persistent: false,
        });

        // let resolved_gizmos_depth_texture;
        let sampled_target;

        if self.msaa.is_some() {
            sampled_target = render_graph.resources.insert_texture(ManagedTextureDesc {
                label: "hrd_output".into(),
                extent,
                dimension: wgpu::TextureDimension::D2,
                format: target_format,
                mip_level_count: 1,
                sample_count,
                persistent: false,
            });

            // resolved_gizmos_depth_texture =
            //     render_graph.resources.insert_texture(ManagedTextureDesc {
            //         label: "gizmos_depth_texture".into(),
            //         extent,
            //         dimension: wgpu::TextureDimension::D2,
            //         format: wgpu::TextureFormat::R32Float,
            //         mip_level_count: 1,
            //         sample_count: 1,
            //         persistent: false,
            //     })
        } else {
            sampled_target = final_color;
            // resolved_gizmos_depth_texture = gizmos_depth_texture;
        };

        let (shadow_maps, shadow_camera_buffer) = match &self.shadow_map_config {
            Some(v) => {
                let shadow_maps = render_graph.resources.insert_texture(ManagedTextureDesc {
                    label: "depth_texture".into(),
                    extent: wgpu::Extent3d {
                        width: v.resolution,
                        height: v.resolution,
                        depth_or_array_layers: v.max_shadows * v.max_cascades,
                    },
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Depth24Plus,
                    mip_level_count: 1,
                    sample_count: 1,
                    persistent: false,
                });

                let shadow_camera_buffer = render_graph.resources.insert_buffer(BufferDesc {
                    label: "shadow_camera_buffer".into(),
                    size: size_of::<LightShadowCamera>() as u64
                        * v.max_shadows as u64
                        * v.max_cascades as u64,
                    usage: BufferUsages::STORAGE,
                });

                (shadow_maps, shadow_camera_buffer)
            }
            None => {
                let shadow_maps = render_graph.resources.insert_texture(ManagedTextureDesc {
                    label: "depth_texture".into(),
                    extent: wgpu::Extent3d {
                        width: 1,
                        height: 1,
                        depth_or_array_layers: 1,
                    },
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Depth24Plus,
                    mip_level_count: 1,
                    sample_count: 1,
                    persistent: false,
                });

                let shadow_camera_buffer = render_graph.resources.insert_buffer(BufferDesc {
                    label: "shadow_camera_buffer".into(),
                    size: size_of::<LightShadowCamera>() as u64,
                    usage: BufferUsages::STORAGE,
                });

                (shadow_maps, shadow_camera_buffer)
            }
        };

        if let Some(shadow_map_config) = self.shadow_map_config {
            render_graph.add_node(ShadowMapNode::new(
                gpu,
                shadow_maps,
                shadow_camera_buffer,
                shadow_map_config.max_shadows as _,
                shadow_map_config.max_cascades as _,
                render_graph.resources.shader_library().clone(),
                object_manager.clone(),
            ));
        }

        let skybox_textures = match self.skybox {
            Some(v) => {
                const MAX_REFLECTION_LOD: u32 = 8;
                let hdri_processor = HdriProcessor::new(gpu, v.format, MAX_REFLECTION_LOD);

                let environment_map = render_graph.resources.insert_texture(ManagedTextureDesc {
                    label: "hdr_cubemap".into(),
                    extent: Extent3d {
                        width: 4098,
                        height: 4098,
                        depth_or_array_layers: 6,
                    },
                    mip_level_count: max_mip_levels(4098, 4098),
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: hdri_processor.format(),
                    persistent: true,
                });

                let irradiance_map = render_graph.resources.insert_texture(ManagedTextureDesc {
                    label: "skybox_ir".into(),
                    extent: Extent3d {
                        width: 512,
                        height: 512,
                        depth_or_array_layers: 6,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: hdri_processor.format(),
                    persistent: true,
                });

                let specular_map = render_graph.resources.insert_texture(ManagedTextureDesc {
                    label: "hdr_cubemap".into(),
                    extent: Extent3d {
                        width: 1024,
                        height: 1024,
                        depth_or_array_layers: 6,
                    },
                    mip_level_count: MAX_REFLECTION_LOD,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: hdri_processor.format(),
                    persistent: true,
                });

                let integrated_brdf = render_graph.resources.insert_texture(ManagedTextureDesc {
                    label: "integrated_brdf".into(),
                    extent: Extent3d {
                        width: 1024,
                        height: 1024,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: target_format,
                    persistent: true,
                });

                let skybox = SkyboxTextures::new(
                    environment_map,
                    irradiance_map,
                    specular_map,
                    integrated_brdf,
                );

                let hdri = v.hdri;
                let assets = assets.clone();
                render_graph.add_node(HdriProcessorNode::new(
                    hdri_processor,
                    stream::once(async move {
                        match hdri.load_async(&assets).await {
                            Ok(v) => Some(v),
                            Err(err) => {
                                tracing::error!(
                                    "{:?}",
                                    anyhow::Error::from(err).context("Failed to load hdri")
                                );
                                None
                            }
                        }
                    })
                    .filter_map(ready)
                    .boxed(),
                    skybox,
                ));
                Some(skybox)
            }
            None => None,
        };

        let camera_renderers = (
            SkyboxRenderer::new(gpu),
            MeshRenderer::new(
                world,
                assets,
                gpu,
                forward_pass(),
                render_graph.resources.shader_library().clone(),
            ),
            MeshRenderer::new(
                world,
                assets,
                gpu,
                transparent_pass(),
                render_graph.resources.shader_library().clone(),
            ),
        );

        let light_manager = LightManager::new(gpu, shadow_maps, shadow_camera_buffer, 16);

        render_graph.add_node(CameraNode::new(
            gpu,
            depth_texture,
            sampled_target,
            camera_renderers,
            light_manager,
            object_manager,
            skybox_textures,
        ));

        let mut last_output = sampled_target;

        let mut screensized = vec![depth_texture, gizmos_depth_texture];

        if needs_indirection_target {
            screensized.push(final_color);
        }

        if self.msaa.is_some() {
            screensized.push(sampled_target);
            screensized.push(resolved_depth_texture);
            // screensized.push(resolved_gizmos_depth_texture);
        };

        render_graph.add_node(MsaaDepthResolve::new(
            gpu,
            depth_texture,
            resolved_depth_texture,
        ));

        if self.msaa.is_some() {
            render_graph.add_node(MsaaResolve::new(sampled_target, final_color));
            last_output = final_color;
        }

        // Apply post-processing effects in order
        for (i, effect) in self.post_processing_effects.iter().enumerate() {
            let effect_result = render_graph.resources.insert_texture(ManagedTextureDesc {
                label: format!("{}_effect_{}", self.label, i).into(),
                extent,
                dimension: wgpu::TextureDimension::D2,
                format: TextureFormat::Rgba16Float,
                mip_level_count: 1,
                sample_count: 1,
                persistent: false,
            });

            effect.add_to_graph(
                gpu,
                render_graph,
                last_output,
                effect_result,
                Some(resolved_depth_texture),
            );

            last_output = effect_result;
            screensized.push(effect_result);
        }

        // Needs resolve to tonemap and write to non-hdr output
        if needs_indirection_target {
            render_graph.add_node(TonemapNode::new_with_grading(
                gpu,
                last_output,
                destination,
                self.color_grading,
            ));
        }

        // working in non-hdr space
        render_graph.add_node(GizmosRendererNode::new(
            gpu,
            destination,
            gizmos_depth_texture,
        ));

        let ui_instance = world.get_clone(engine(), ui_instance()).ok();
        if let Some(ui) = ui_instance {
            render_graph.add_node(UiRenderNode::new(gpu, ui, destination));
        }

        PbrRenderGraphTextures { screensized }
    }
}

impl PbrRenderGraphTextures {
    pub fn set_size(&self, render_graph: &mut RenderGraph, size: PhysicalSize<u32>) {
        let new_extent = Extent3d {
            width: size.width,
            height: size.height,
            depth_or_array_layers: 1,
        };

        for &handle in self.screensized() {
            render_graph
                .resources
                .get_texture_mut(handle)
                .as_managed_mut()
                .unwrap()
                .extent = new_extent;
        }
    }
}
