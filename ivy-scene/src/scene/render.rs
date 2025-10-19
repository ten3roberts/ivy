use std::mem;

use flax::{Entity, World};
use ivy_assets::{stored::DynamicStore, AssetCache, AssetPath};
use ivy_postprocessing::preconfigured::pbr::{
    PbrRenderGraphConfig, PbrRenderGraphTextures, SkyboxConfig,
};
use ivy_wgpu::{
    rendergraph::{
        self, Dependency, ExternalResources, Node, NodeExecutionContext, NodeUpdateContext,
        RenderGraph, TextureHandle, UpdateResult,
    },
    types::PhysicalSize,
    Gpu,
};
use wgpu::{TextureFormat, TextureUsages};

use super::scene_world;

/// Renders another scene into a texture
pub struct SceneRenderNode {
    world_id: Entity,
    // node to execute within the scene
    subgraph: RenderGraph,
    view_handle: TextureHandle,

    subgraph_output: TextureHandle,
    pbr: PbrRenderGraphTextures,
    update_size: bool,
}

impl SceneRenderNode {
    pub fn new(
        engine_world: &mut World,
        gpu: &Gpu,
        assets: &AssetCache,
        store: &mut DynamicStore,
        world_id: Entity,
        mut subgraph: RenderGraph,
        view_handle: TextureHandle,
    ) -> Self {
        let subgraph_resources = &mut subgraph.resources;

        // Create an output texture that the scene is rendered into.
        //
        // This will be exposed to the main rendergraph and can be stitched into the UI
        let subgraph_output =
            subgraph_resources.insert_texture(rendergraph::RenderGraphImageDesc::External);

        let mut scene_world = engine_world
            .get_mut(world_id, scene_world())
            .expect("Missing scene");

        let pbr = get_rendering_settings().configure(
            &mut scene_world,
            gpu,
            assets,
            store,
            &mut subgraph,
            subgraph_output,
        );

        pbr.set_size(
            &mut subgraph,
            PhysicalSize {
                width: 480,
                height: 480,
            },
        );

        Self {
            view_handle,
            world_id,
            subgraph,
            subgraph_output,
            pbr,
            update_size: true,
        }
    }
}

impl Node for SceneRenderNode {
    fn label(&self) -> &str {
        std::any::type_name::<Self>()
    }

    fn update(&mut self, ctx: NodeUpdateContext) -> anyhow::Result<UpdateResult> {
        let scene_world = &mut *ctx.world.get_mut(self.world_id, scene_world())?;

        let view_texture = ctx.get_texture(self.view_handle);

        let mut external_resources = ExternalResources::new();
        external_resources.insert_texture(self.subgraph_output, view_texture);

        if mem::take(&mut self.update_size) {
            self.pbr.set_size(
                &mut self.subgraph,
                PhysicalSize {
                    width: view_texture.width(),
                    height: view_texture.height(),
                },
            );
        }

        self.subgraph.update(
            ctx.gpu,
            scene_world,
            ctx.assets,
            ctx.store,
            &external_resources,
        )?;

        Ok(UpdateResult::Success)
    }

    fn draw(&mut self, ctx: NodeExecutionContext) -> anyhow::Result<()> {
        let scene_world = &mut *ctx.world.get_mut(self.world_id, scene_world())?;

        let view_texture = ctx.get_texture(self.view_handle);

        let mut external_resources = ExternalResources::new();
        external_resources.insert_texture(self.subgraph_output, view_texture);

        self.subgraph.draw_with_encoder(
            ctx.gpu,
            ctx.queue,
            ctx.encoder,
            scene_world,
            ctx.assets,
            ctx.store,
            &external_resources,
        )?;

        Ok(())
    }

    fn on_resource_changed(&mut self, resource: ivy_wgpu::rendergraph::ResourceHandle) {
        if resource.as_texture() == Some(&self.view_handle) {
            self.update_size = true;
        }
    }

    fn read_dependencies(&self) -> Vec<Dependency> {
        vec![]
    }

    fn write_dependencies(&self) -> Vec<Dependency> {
        vec![Dependency::texture(
            self.view_handle,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
        )]
    }
}

fn get_rendering_settings() -> PbrRenderGraphConfig {
    const SIMPLE_RENDERING: bool = false;

    if SIMPLE_RENDERING {
        PbrRenderGraphConfig {
            shadow_map_config: None,
            msaa: None,
            bloom: None,
            dof: None,
            skybox: None,
            hdr_format: None,
            label: "scene".into(),
        }
    } else {
        PbrRenderGraphConfig {
            shadow_map_config: Some(Default::default()),
            msaa: Some(Default::default()),
            bloom: Some(Default::default()),
            dof: Some(Default::default()),
            skybox: Some(SkyboxConfig {
                hdri: Box::new(AssetPath::new(
                    // "hdris/kloofendal_48d_partly_cloudy_puresky_2k.hdr",
                    "hdris/lauter_waterfall_4k.hdr",
                )),
                format: TextureFormat::Rgba16Float,
            }),
            hdr_format: Some(wgpu::TextureFormat::Rgba16Float),
            label: "scene".into(),
        }
    }
}
