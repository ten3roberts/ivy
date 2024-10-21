use std::{future::ready, mem::size_of, sync::Arc};

use flax::World;
use futures::{stream, StreamExt};
use image::DynamicImage;
use ivy_assets::{AssetCache, AsyncAssetKey};
use ivy_ui::{node::UiRenderNode, SharedUiInstance};
use ivy_wgpu::{
    components::forward_pass,
    renderer::{
        gizmos_renderer::GizmosRendererNode,
        mesh_renderer::MeshRenderer,
        shadowmapping::{LightShadowCamera, ShadowMapNode},
        skinned_mesh_renderer::SkinnedMeshRenderer,
        CameraNode, LightManager, MsaaResolve, SkyboxTextures,
    },
    rendergraph::{BufferDesc, ManagedTextureDesc, RenderGraph, TextureHandle},
    shader_library::ShaderLibrary,
    types::{texture::max_mip_levels, PhysicalSize},
    Gpu,
};
use wgpu::{BufferUsages, Extent3d, TextureDimension, TextureFormat};

use crate::{
    bloom::BloomNode,
    depth_resolve::MsaaDepthResolve,
    hdri::{HdriProcessor, HdriProcessorNode},
    skybox::SkyboxRenderer,
    tonemap::TonemapNode,
};

/// Pre-configured render graph suited for PBR render pipelines
pub struct PbrRenderGraphConfig {
    pub shadow_map_config: Option<ShadowMapConfig>,
    pub msaa: Option<MsaaConfig>,
    pub bloom: Option<BloomConfig>,
    pub skybox: Option<SkyboxConfig>,
    pub hdr_format: TextureFormat,
}

pub struct SkyboxConfig {
    pub hdri: Box<dyn AsyncAssetKey<DynamicImage>>,
    pub format: TextureFormat,
}

#[derive(Debug, Clone)]
pub struct ShadowMapConfig {
    pub resolution: u32,
    pub max_cascades: u32,
    pub max_shadows: u32,
}

impl Default for ShadowMapConfig {
    fn default() -> Self {
        Self {
            resolution: 2048,
            max_cascades: 4,
            max_shadows: 4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MsaaConfig {
    pub sample_count: u32,
}

impl Default for MsaaConfig {
    fn default() -> Self {
        Self { sample_count: 4 }
    }
}

#[derive(Debug, Clone)]
pub struct BloomConfig {
    pub filter_radius: f32,
    pub layers: u32,
}

impl Default for BloomConfig {
    fn default() -> Self {
        Self {
            filter_radius: 0.002,
            layers: 4,
        }
    }
}

pub struct PbrRenderGraph {
    screensized: Vec<TextureHandle>,
}

impl PbrRenderGraph {
    pub fn screensized(&self) -> &[TextureHandle] {
        &self.screensized
    }
}

impl PbrRenderGraphConfig {
    pub fn configure(
        self,
        world: &mut World,
        gpu: &Gpu,
        assets: &AssetCache,
        render_graph: &mut RenderGraph,
        shader_library: Arc<ShaderLibrary>,
        ui_instance: Option<SharedUiInstance>,
        destination: TextureHandle,
    ) -> PbrRenderGraph {
        let extent = Extent3d {
            width: 0,
            height: 0,
            depth_or_array_layers: 1,
        };

        let hdr_format = self.hdr_format;

        let final_color = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "final_color".into(),
            extent,
            dimension: wgpu::TextureDimension::D2,
            format: hdr_format,
            mip_level_count: 1,
            sample_count: 1,
            persistent: false,
        });

        let sample_count = self.msaa.as_ref().map(|v| v.sample_count).unwrap_or(1);

        let hdr_output = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "hrd_output".into(),
            extent,
            dimension: wgpu::TextureDimension::D2,
            format: hdr_format,
            mip_level_count: 1,
            sample_count,
            persistent: false,
        });

        let depth_texture = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "depth_texture".into(),
            extent,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            mip_level_count: 1,
            sample_count,
            persistent: false,
        });

        let resolved_depth_texture = if self.msaa.is_some() {
            render_graph.resources.insert_texture(ManagedTextureDesc {
                label: "depth_texture".into(),
                extent,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R32Float,
                mip_level_count: 1,
                sample_count: 1,
                persistent: false,
            })
        } else {
            depth_texture
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
                world,
                gpu,
                shadow_maps,
                shadow_camera_buffer,
                shadow_map_config.max_shadows as _,
                shadow_map_config.max_cascades as _,
                shader_library.clone(),
            ));
        }

        let skybox_textures = match self.skybox {
            Some(v) => {
                const MAX_REFLECTION_LOD: u32 = 8;
                let hdri_processor = HdriProcessor::new(gpu, v.format, MAX_REFLECTION_LOD);

                let environment_map = render_graph.resources.insert_texture(ManagedTextureDesc {
                    label: "hdr_cubemap".into(),
                    extent: Extent3d {
                        width: 1024,
                        height: 1024,
                        depth_or_array_layers: 6,
                    },
                    mip_level_count: max_mip_levels(1024, 1024),
                    sample_count: 1,
                    dimension: TextureDimension::D2,
                    format: hdri_processor.format(),
                    persistent: true,
                });

                let irradiance_map = render_graph.resources.insert_texture(ManagedTextureDesc {
                    label: "skybox_ir".into(),
                    extent: Extent3d {
                        width: 256,
                        height: 256,
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
                    format: hdr_format,
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
            MeshRenderer::new(world, gpu, forward_pass(), shader_library.clone()),
            SkinnedMeshRenderer::new(world, gpu, forward_pass(), shader_library.clone()),
        );

        let light_manager = LightManager::new(gpu, shadow_maps, shadow_camera_buffer, 4);

        render_graph.add_node(CameraNode::new(
            gpu,
            depth_texture,
            hdr_output,
            camera_renderers,
            light_manager,
            skybox_textures,
        ));

        let mut last_output = hdr_output;

        let mut screensized = vec![
            hdr_output,
            final_color,
            depth_texture,
            resolved_depth_texture,
        ];

        if self.msaa.is_some() {
            render_graph.add_node(MsaaResolve::new(hdr_output, final_color));
            render_graph.add_node(MsaaDepthResolve::new(
                gpu,
                depth_texture,
                resolved_depth_texture,
            ));
            last_output = final_color;
        }

        if let Some(bloom) = self.bloom {
            let bloom_result = render_graph.resources.insert_texture(ManagedTextureDesc {
                label: "bloom_result".into(),
                extent,
                dimension: wgpu::TextureDimension::D2,
                format: TextureFormat::Rgba16Float,
                mip_level_count: 1,
                sample_count: 1,
                persistent: false,
            });

            render_graph.add_node(BloomNode::new(
                gpu,
                last_output,
                bloom_result,
                bloom.layers,
                bloom.filter_radius,
            ));

            last_output = bloom_result;

            screensized.push(bloom_result);
        }

        render_graph.add_node(TonemapNode::new(gpu, last_output, destination));

        render_graph.add_node(GizmosRendererNode::new(
            gpu,
            destination,
            resolved_depth_texture,
        ));

        if let Some(ui) = ui_instance {
            render_graph.add_node(UiRenderNode::new(gpu, ui, destination))
        }

        PbrRenderGraph { screensized }
    }
}

impl PbrRenderGraph {
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
