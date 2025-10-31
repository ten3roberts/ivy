pub mod pbr;

use std::sync::Arc;

use anyhow::Context;
use flax::World;
use ivy_assets::{
    stored::{DynamicStore, Handle},
    AssetCache,
};
use ivy_core::profiling::profile_scope;
use ivy_ui::components::ui_instance;
use ivy_wgpu::{
    rendergraph::{self, ExternalResources, RenderGraph, RenderGraphResources, TextureHandle},
    shader_library::{ShaderLibrary, ShaderModuleDesc},
    types::{PhysicalSize, Surface},
    Gpu,
};
use pbr::PbrRenderGraphConfig;

use crate::preconfigured::pbr::PbrRenderGraphTextures;

#[derive(Default)]
pub struct SurfacePbrPipelineDesc {
    pub pbr_config: PbrRenderGraphConfig,
}

/// Uses a rendergraph to render to a surface
pub struct SurfacePbrRenderer {
    render_graph: Handle<RenderGraph>,
    surface: Surface,
    surface_texture: rendergraph::TextureHandle,
    pbr: PbrRenderGraphTextures,
}

impl SurfacePbrRenderer {
    pub fn new(
        world: &mut World,
        assets: &AssetCache,
        store: &mut DynamicStore,
        gpu: &Gpu,
        surface: Surface,
        desc: SurfacePbrPipelineDesc,
    ) -> Self {
        // TODO; pass as param
        let shader_library = ShaderLibrary::new()
            .with_module(ShaderModuleDesc {
                path: "./assets/shaders/pbr_base.wgsl",
                source: include_str!("../../../assets/shaders/pbr_base.wgsl"),
                shader_defs: Default::default(),
            })
            .with_module(ShaderModuleDesc {
                path: "./assets/shaders/vertex.wgsl",
                source: include_str!("../../../assets/shaders/vertex.wgsl"),
                shader_defs: Default::default(),
            })
            .with_module(ShaderModuleDesc {
                path: "./assets/shaders/material_pbr.wgsl",
                source: include_str!("../../../assets/shaders/material_pbr.wgsl"),
                shader_defs: Default::default(),
            });

        let shader_library = Arc::new(shader_library);

        let resources = RenderGraphResources::new(shader_library.clone());
        let mut render_graph = RenderGraph::new(resources);

        let surface_texture = render_graph
            .resources
            .insert_texture(rendergraph::RenderGraphImageDesc::External);

        let pbr = desc.pbr_config.configure(
            world,
            gpu,
            assets,
            store,
            &mut render_graph,
            surface_texture,
        );

        let render_graph = store.insert(render_graph);

        Self {
            render_graph,
            surface,
            surface_texture,
            pbr,
        }
    }
}

impl ivy_wgpu::layer::Renderer for SurfacePbrRenderer {
    fn render_graph(&self) -> ivy_assets::stored::Handle<RenderGraph> {
        self.render_graph.clone()
    }

    fn draw(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        store: &mut DynamicStore,
        gpu: &Gpu,
        queue: &wgpu::Queue,
    ) -> anyhow::Result<()> {
        let surface_texture = self.surface.get_current_texture()?;

        let mut external_resources = ExternalResources::new();
        external_resources.insert_texture(self.surface_texture, &surface_texture.texture);
        let render_graph = &mut *store.get_mut(&self.render_graph);

        render_graph.update(gpu, world, assets, store, &external_resources)?;

        let mut encoder = gpu.device.create_command_encoder(&Default::default());

        render_graph.draw_with_encoder(
            gpu,
            queue,
            &mut encoder,
            world,
            assets,
            store,
            &external_resources,
        )?;

        {
            profile_scope!("submit");
            gpu.queue.submit([encoder.finish()]);
        }

        {
            profile_scope!("present");
            surface_texture.present();
        }

        Ok(())
    }

    fn on_resize(&mut self, gpu: &Gpu, store: &DynamicStore, size: PhysicalSize<u32>) {
        self.surface.resize(gpu, size);

        self.pbr
            .set_size(&mut *store.get_mut(&self.render_graph), size);
    }

    // fn process_commands(
    //     &mut self,
    //     world: &mut World,
    //     assets: &AssetCache,
    //     store: &mut DynamicStore,
    //     gpu: &Gpu,
    //     cmds: &mut flume::Receiver<ivy_wgpu::layer::RendererCommand>,
    // ) -> anyhow::Result<()> {
    //     for cmd in cmds.drain() {
    //         match cmd {
    //             ivy_wgpu::layer::RendererCommand::ModifyRenderGraph(func) => {
    //                 func(world, assets, store, gpu, &mut self.render_graph)?;
    //             }
    //             ivy_wgpu::layer::RendererCommand::UpdateTexture { handle, desc } => {
    //                 *self
    //                     .render_graph
    //                     .resources
    //                     .get_texture_mut(handle)
    //                     .as_managed_mut()
    //                     .context("Attempt to modify an external texture")? = desc;
    //             }
    //         }
    //     }

    //     Ok(())
    // }
}

pub struct SurfacePipelineDesc {}

/// Uses a rendergraph to render to a surface
pub struct SurfaceRenderer {
    render_graph: Handle<RenderGraph>,
    surface: Surface,
    surface_handle: rendergraph::TextureHandle,
}

impl SurfaceRenderer {
    pub fn new(store: &mut DynamicStore, surface: Surface) -> Self {
        // TODO; pass as param
        let shader_library = ShaderLibrary::new()
            .with_module(ShaderModuleDesc {
                path: "./assets/shaders/pbr_base.wgsl",
                source: include_str!("../../../assets/shaders/pbr_base.wgsl"),
                shader_defs: Default::default(),
            })
            .with_module(ShaderModuleDesc {
                path: "./assets/shaders/vertex.wgsl",
                source: include_str!("../../../assets/shaders/vertex.wgsl"),
                shader_defs: Default::default(),
            })
            .with_module(ShaderModuleDesc {
                path: "./assets/shaders/material_pbr.wgsl",
                source: include_str!("../../../assets/shaders/material_pbr.wgsl"),
                shader_defs: Default::default(),
            });

        let shader_library = Arc::new(shader_library);

        let resources = RenderGraphResources::new(shader_library.clone());
        let mut render_graph = RenderGraph::new(resources);

        let surface_texture = render_graph
            .resources
            .insert_texture(rendergraph::RenderGraphImageDesc::External);

        Self {
            render_graph: store.insert(render_graph),
            surface,
            surface_handle: surface_texture,
        }
    }

    pub fn surface_handle(&self) -> TextureHandle {
        self.surface_handle
    }
}

impl ivy_wgpu::layer::Renderer for SurfaceRenderer {
    fn render_graph(&self) -> Handle<RenderGraph> {
        self.render_graph.clone()
    }

    fn draw(
        &mut self,
        world: &mut World,
        assets: &AssetCache,
        store: &mut DynamicStore,
        gpu: &Gpu,
        queue: &wgpu::Queue,
    ) -> anyhow::Result<()> {
        let surface_texture = self.surface.get_current_texture()?;

        let mut external_resources = ExternalResources::new();
        external_resources.insert_texture(self.surface_handle, &surface_texture.texture);

        let render_graph = &mut *store.get_mut(&self.render_graph);
        render_graph.update(gpu, world, assets, store, &external_resources)?;

        let mut encoder = gpu.device.create_command_encoder(&Default::default());

        render_graph.draw_with_encoder(
            gpu,
            queue,
            &mut encoder,
            world,
            assets,
            store,
            &external_resources,
        )?;

        {
            profile_scope!("submit");
            gpu.queue.submit([encoder.finish()]);
        }

        {
            profile_scope!("present");
            surface_texture.present();
        }

        Ok(())
    }

    fn on_resize(&mut self, gpu: &Gpu, store: &DynamicStore, size: PhysicalSize<u32>) {
        self.surface.resize(gpu, size);
    }

    // fn process_commands(
    //     &mut self,
    //     world: &mut World,
    //     assets: &AssetCache,
    //     store: &mut DynamicStore,
    //     gpu: &Gpu,
    //     cmds: &mut flume::Receiver<ivy_wgpu::layer::RendererCommand>,
    // ) -> anyhow::Result<()> {
    //     for cmd in cmds.drain() {
    //         match cmd {
    //             ivy_wgpu::layer::RendererCommand::ModifyRenderGraph(func) => {
    //                 func(world, assets, store, gpu, &mut self.render_graph)?;
    //             }
    //             ivy_wgpu::layer::RendererCommand::UpdateTexture { handle, desc } => {
    //                 *self
    //                     .render_graph
    //                     .resources
    //                     .get_texture_mut(handle)
    //                     .as_managed_mut()
    //                     .context("Attempt to modify an external texture")? = desc;
    //             }
    //         }
    //     }

    //     Ok(())
    // }
}
