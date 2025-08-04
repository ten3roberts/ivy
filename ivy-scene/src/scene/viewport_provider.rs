use flax::{Entity, World};
use ivy_core::components::engine;
use ivy_ui::{screens::screen_state, streamed::streamed_tx};
use ivy_wgpu::{
    layer::{renderer_commands, RendererCommand},
    rendergraph::{RenderGraph, RenderGraphResources},
};

use crate::scene_world;

/// Hooks into a new scene and provides it with a viewport
pub struct SceneViewportProvider {
    scene_view_commands_tx: flume::Sender<SceneViewCommand>,
    proxy_nodes: Arc<Mutex<BTreeMap<Entity, (NodeId, TextureHandle)>>>,
}

impl SceneViewportProvider {
    pub fn new(scene_view_commands_tx: flume::Sender<SceneViewCommand>) -> Self {
        Self {
            scene_view_commands_tx,
            proxy_nodes: Default::default(),
        }
    }

    fn process_new_scene(&mut self, engine_world: &World, scene_id: Entity) -> anyhow::Result<()> {
        let renderer_commands = engine_world.get(engine(), renderer_commands())?.clone();
        let renderer_commands2 = renderer_commands.clone();

        let scene_view_commands = self.scene_view_commands_tx.clone();
        let proxy_nodes = self.proxy_nodes.clone();

        if proxy_nodes.lock().contains_key(&scene_id) {
            return Ok(());
        }

        let scene = engine_world.get_mut(scene_id, scene_world())?;
        let streamed_tx = scene.get(engine(), streamed_tx())?.clone();
        let screen_state = scene.get(engine(), screen_state())?.clone();

        // TODO: thread local storage for non-callback access
        let renderer_command = RendererCommand::modify_rendergraph(
            move |engine_world, assets, store, gpu, render_graph| {
                let subgraph_resources =
                    RenderGraphResources::new(render_graph.resources.shader_library().clone());
                let subgraph = RenderGraph::new(subgraph_resources);

                let mut scene_render_desc = ManagedTextureDesc {
                    label: "scene_dst".into(),
                    extent: wgpu::Extent3d {
                        width: 240,
                        height: 240,
                        depth_or_array_layers: 1,
                    },
                    dimension: TextureDimension::D2,
                    format: TextureFormat::Rgba8UnormSrgb,
                    mip_level_count: 1,
                    sample_count: 1,
                    persistent: false,
                };

                let scene_render = render_graph
                    .resources
                    .insert_texture(rendergraph::TextureDesc::Managed(scene_render_desc.clone()));

                let node = ProxyNode::new(
                    engine_world,
                    gpu,
                    assets,
                    store,
                    scene_id,
                    subgraph,
                    scene_render,
                );

                let node_id = render_graph.add_node(node);

                proxy_nodes.lock().insert(scene_id, (node_id, scene_render));

                let on_size = move |size: Vec2| {
                    let px = size.as_uvec2();

                    scene_render_desc.extent.width = px.x;
                    scene_render_desc.extent.height = px.y;

                    let _ = renderer_commands2.send(RendererCommand::UpdateTexture {
                        handle: scene_render,
                        desc: scene_render_desc.clone(),
                    });
                };

                let _ = scene_view_commands.send(SceneViewCommand::OpenViewport {
                    scene: scene_id,
                    view: scene_render,
                    on_size: Box::new(on_size),
                    streamed_tx,
                    screen_state,
                });

                Ok(())
            },
        );

        renderer_commands.send(renderer_command)?;

        Ok(())
    }

    fn process_removed_scene(
        &mut self,
        engine_world: &World,
        scene_id: Entity,
    ) -> anyhow::Result<()> {
        let renderer_commands = engine_world.get(engine(), renderer_commands())?.clone();

        let scene_view_commands = self.scene_view_commands_tx.clone();

        let (id, texture_handle) = self
            .proxy_nodes
            .lock()
            .remove(&scene_id)
            .context("Scene was never added")?;

        // TODO: thread local storage for non-callback access
        let renderer_command =
            RendererCommand::modify_rendergraph(move |_, _, _, _, render_graph| {
                render_graph.remove_node(id).context("Missing proxy node")?;
                render_graph
                    .resources
                    .remove_texture(texture_handle)
                    .context("Missing texture for proxy node")?;

                Ok(())
            });

        let _ = scene_view_commands.send(SceneViewCommand::CloseViewport { scene: scene_id });
        renderer_commands.send(renderer_command)?;

        Ok(())
    }
}
