pub mod pbr;

use std::sync::Arc;

use flax::World;
use image::DynamicImage;
use ivy_assets::{AssetCache, AsyncAssetKey};
use ivy_core::profiling::profile_scope;
use ivy_ui::SharedUiInstance;
use ivy_wgpu::{
    rendergraph::{self, ExternalResources, RenderGraph},
    shader_library::{ModuleDesc, ShaderLibrary},
    types::{PhysicalSize, Surface},
    Gpu,
};
use pbr::{PbrRenderGraph, PbrRenderGraphConfig, SkyboxConfig};

pub struct SurfacePbrPipelineDesc {
    pub hdri: Option<Box<dyn AsyncAssetKey<DynamicImage>>>,
    /// Render Ui if configured
    pub ui_instance: Option<SharedUiInstance>,
}

pub struct SurfacePbrPipeline {
    render_graph: RenderGraph,
    surface: Surface,
    surface_texture: rendergraph::TextureHandle,
    pbr: PbrRenderGraph,
}

impl SurfacePbrPipeline {
    pub fn new(
        world: &mut World,
        assets: &AssetCache,
        gpu: &Gpu,
        surface: Surface,
        desc: SurfacePbrPipelineDesc,
    ) -> Self {
        let mut render_graph = RenderGraph::new();

        let surface_texture = render_graph
            .resources
            .insert_texture(rendergraph::TextureDesc::External);

        let shader_library = ShaderLibrary::new().with_module(ModuleDesc {
            path: "./assets/shaders/pbr_base.wgsl",
            source: &assets.load::<String>(&"shaders/pbr_base.wgsl".to_string()),
        });

        let shader_library = Arc::new(shader_library);

        let pbr = PbrRenderGraphConfig {
            shadow_map_config: Some(Default::default()),
            msaa: Some(Default::default()),
            bloom: None,
            skybox: desc.hdri.map(|v| SkyboxConfig {
                hdri: v,
                format: wgpu::TextureFormat::Rgba16Float,
            }),
            hdr_format: wgpu::TextureFormat::Rgba16Float,
        }
        .configure(
            world,
            gpu,
            assets,
            &mut render_graph,
            shader_library.clone(),
            desc.ui_instance,
            surface_texture,
        );

        Self {
            render_graph,
            surface,
            surface_texture,
            pbr,
        }
    }
}

impl ivy_wgpu::layer::Renderer for SurfacePbrPipeline {
    fn draw(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        gpu: &Gpu,
        queue: &wgpu::Queue,
    ) -> anyhow::Result<()> {
        let surface_texture = self.surface.get_current_texture()?;

        let mut external_resources = ExternalResources::new();
        external_resources.insert_texture(self.surface_texture, &surface_texture.texture);

        self.render_graph
            .draw(gpu, queue, world, assets, &external_resources)?;

        {
            profile_scope!("present");
            surface_texture.present();
        }

        Ok(())
    }

    fn on_resize(&mut self, gpu: &Gpu, size: PhysicalSize<u32>) {
        self.surface.resize(gpu, size);

        self.pbr.set_size(&mut self.render_graph, size);
    }
}
