use std::{mem::size_of, sync::Arc};

use async_std::path::PathBuf;
use flax::World;
use glam::Mat4;
use image::DynamicImage;
use ivy_assets::DynAssetDesc;
use ivy_wgpu::{
    components::forward_pass,
    renderer::{
        mesh_renderer::MeshRenderer, shadowmapping::ShadowMapNode,
        skinned_mesh_renderer::SkinnedMeshRenderer, CameraNode, LightManager,
    },
    rendergraph::{BufferDesc, ManagedTextureDesc, RenderGraph, TextureHandle},
    shader_library::{self, ShaderLibrary},
    Gpu,
};
use wgpu::{BufferUsages, Extent3d, TextureFormat};

use crate::{bloom::BloomNode, skybox::SkyboxRenderer, tonemap::TonemapNode};

/// Pre-configured rendergraph suited for PBR render pipelines
pub struct PbrRenderGraphConfig {
    shadow_map_config: Option<ShadowMapConfig>,
    msaa: Option<MsaaConfig>,
    bloom: Option<BloomConfig>,
    skybox: Option<SkyboxConfig>,
}

pub struct SkyboxConfig {
    hdri: Box<dyn DynAssetDesc<DynamicImage>>,
}

#[derive(Debug, Clone)]
pub struct ShadowMapConfig {
    resolution: u32,
    max_cascades: u32,
    max_shadows: u32,
}

impl Default for ShadowMapConfig {
    fn default() -> Self {
        Self {
            resolution: 1024,
            max_cascades: 4,
            max_shadows: 4,
        }
    }
}

#[derive(Debug, Clone)]
pub struct MsaaConfig {
    sample_count: u32,
}

impl Default for MsaaConfig {
    fn default() -> Self {
        Self { sample_count: 4 }
    }
}

#[derive(Debug, Clone)]
pub struct BloomConfig {
    filter_radius: f32,
    layers: u32,
}

impl Default for BloomConfig {
    fn default() -> Self {
        Self {
            filter_radius: 0.002,
            layers: 4,
        }
    }
}

enum PostProcessingEffect {
    Bloom(BloomConfig),
}

pub struct PbrRenderGraph {}

impl PbrRenderGraphConfig {
    pub fn configure(
        self,
        world: &mut World,
        gpu: &Gpu,
        render_graph: &mut RenderGraph,
        shader_library: Arc<ShaderLibrary>,
        extent: Extent3d,
        destination: TextureHandle,
    ) -> PbrRenderGraph {
        let hdr_format = TextureFormat::Rgba16Float;

        let final_color = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "final_color".into(),
            extent,
            dimension: wgpu::TextureDimension::D2,
            format: hdr_format,
            mip_level_count: 1,
            sample_count: 1,
            persistent: false,
        });

        let multisampled_hdr = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "multisampled_hdr".into(),
            extent,
            dimension: wgpu::TextureDimension::D2,
            format: hdr_format,
            mip_level_count: 1,
            sample_count: 4,
            persistent: false,
        });

        let depth_texture = render_graph.resources.insert_texture(ManagedTextureDesc {
            label: "depth_texture".into(),
            extent,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            mip_level_count: 1,
            sample_count: 4,
            persistent: false,
        });

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
                    size: size_of::<Mat4>() as u64 * v.max_shadows as u64 * v.max_cascades as u64,
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
                    size: size_of::<Mat4>() as u64,
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

        let camera_renderers = (
            SkyboxRenderer::new(gpu),
            MeshRenderer::new(world, gpu, forward_pass(), shader_library.clone()),
            SkinnedMeshRenderer::new(world, gpu, forward_pass(), shader_library.clone()),
        );

        let light_manager = LightManager::new(gpu, shadow_maps, shadow_camera_buffer, 4);

        render_graph.add_node(CameraNode::new(
            gpu,
            depth_texture,
            multisampled_hdr,
            camera_renderers,
            light_manager,
            skybox_textures,
        ));

        let mut last_output = final_color;

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
        }

        render_graph.add_node(TonemapNode::new(gpu, last_output, destination));

        todo!()
    }
}
