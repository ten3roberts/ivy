use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use anyhow::Context;
use flax::{
    events::{EventKindFilter, EventSubscriber},
    Entity, World,
};
use glam::Vec2;
use ivy_core::{app::TickEvent, components::engine, Layer};
use ivy_ui::{
    screens::{screen_state, ScreenState},
    streamed::{streamed_tx, Streamed},
    violet::core::to_owned,
};
use ivy_wgpu::{
    layer::{renderer_commands, RendererCommand},
    rendergraph::{
        ManagedTextureDesc, NodeId, RenderGraph, RenderGraphImageDesc, RenderGraphResources,
        TextureHandle,
    },
};
use parking_lot::Mutex;
use wgpu::{TextureDimension, TextureFormat};

use crate::{render::SceneRenderNode, scene_world, ui::SceneViewCommand};

flax::component! {
    /// Allows listening to and receiving new viewports from the scene system
    scene_viewport_state: SceneViewportState,
}

#[derive(Clone)]
pub struct SceneViewportState {
    inner: Arc<Mutex<SceneViewportStateInner>>,
}

struct SceneViewportStateInner {
    new_listeners: Vec<flume::Sender<SceneViewCommand>>,
}

/// Hooks into a new scene and provides it with a viewport
pub struct SceneViewportProvider {
    open_scenes: BTreeSet<Entity>,
    state: SceneViewportState,
    listeners: Vec<flume::Sender<SceneViewCommand>>,
    proxy_nodes:
        Arc<Mutex<BTreeMap<Entity, Vec<(NodeId, TextureHandle, flume::Sender<SceneViewCommand>)>>>>,
}

impl SceneViewportProvider {
    fn process_new_scene(&mut self, engine_world: &World, scene_id: Entity) -> anyhow::Result<()> {
        let renderer_commands = engine_world.get(engine(), renderer_commands())?.clone();
        let proxy_nodes = self.proxy_nodes.clone();

        if proxy_nodes.lock().contains_key(&scene_id) {
            tracing::info!("Duplicate scene viewport request for scene {scene_id:?}");
            return Ok(());
        }

        self.open_scenes.insert(scene_id);

        let scene = engine_world.get_mut(scene_id, scene_world())?;
        let streamed_tx = scene.get(engine(), streamed_tx())?.clone();
        let screen_state = scene.get(engine(), screen_state())?.clone();

        // TODO: thread local storage for non-callback access
        for tx in self.listeners.iter().cloned() {
            to_owned!(scene_id, streamed_tx, screen_state);

            self.open_viewport(&renderer_commands, scene_id, streamed_tx, screen_state, tx)?;
        }

        Ok(())
    }

    fn process_removed_scene(
        &mut self,
        engine_world: &World,
        scene_id: Entity,
    ) -> anyhow::Result<()> {
        let renderer_commands = engine_world.get(engine(), renderer_commands())?.clone();

        self.open_scenes.remove(&scene_id);

        let nodes = self
            .proxy_nodes
            .lock()
            .remove(&scene_id)
            .context("Scene was never added")?;

        // Remove all nodes associated to the old scene for all open viewports
        for (node_id, texture_handle, tx) in nodes {
            // TODO: thread local storage for non-callback access
            let renderer_command =
                RendererCommand::modify_rendergraph(move |_, _, _, _, render_graph| {
                    render_graph
                        .remove_node(node_id)
                        .context("Missing proxy node")?;
                    render_graph
                        .resources
                        .remove_texture(texture_handle)
                        .context("Missing texture for proxy node")?;

                    Ok(())
                });

            let _ = tx.send(SceneViewCommand::CloseViewport { scene: scene_id });
            renderer_commands.send(renderer_command)?;
        }

        Ok(())
    }

    fn open_viewport(
        &self,
        renderer_commands: &flume::Sender<RendererCommand>,
        scene_id: Entity,
        streamed_tx: flume::Sender<Box<dyn Streamed>>,
        screen_state: ScreenState,

        tx: flume::Sender<SceneViewCommand>,
    ) -> Result<(), anyhow::Error> {
        let proxy_nodes = self.proxy_nodes.clone();
        let renderer_commands2 = renderer_commands.clone();
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
                    .insert_texture(RenderGraphImageDesc::Managed(scene_render_desc.clone()));

                let node = SceneRenderNode::new(
                    engine_world,
                    gpu,
                    assets,
                    store,
                    scene_id,
                    subgraph,
                    scene_render,
                );

                let node_id = render_graph.add_node(node);

                proxy_nodes.lock().entry(scene_id).or_default().push((
                    node_id,
                    scene_render,
                    tx.clone(),
                ));

                let on_size = move |size: Vec2| {
                    let px = size.as_uvec2();

                    scene_render_desc.extent.width = px.x;
                    scene_render_desc.extent.height = px.y;

                    // TODO: single frame latency
                    let _ = renderer_commands2.send(RendererCommand::UpdateTexture {
                        handle: scene_render,
                        desc: scene_render_desc.clone(),
                    });
                };

                let _ = tx.send(SceneViewCommand::OpenViewport {
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
}

impl Layer for SceneViewportProvider {
    fn register(
        &mut self,
        world: &mut World,
        _assets: &ivy_assets::AssetCache,
        _store: &mut ivy_assets::stored::DynamicStore,
        mut events: ivy_core::events::EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        let (tx, rx) = flume::unbounded();

        world.set(engine(), scene_viewport_state(), self.state.clone())?;

        world.subscribe(
            tx.filter_arch(scene_world().with())
                .filter_components([scene_world().key()])
                .filter_event_kind(EventKindFilter::ADDED | EventKindFilter::REMOVED),
        );

        events.subscribe(move |this, ctx, _: &TickEvent| {
            for new_listener in this.state.inner.lock().new_listeners.drain(..) {
                for &scene_id in &this.open_scenes {
                    let scene = ctx.world.get_mut(scene_id, scene_world())?;
                    let renderer_commands = ctx.world.get(engine(), renderer_commands())?;
                    let streamed_tx = scene.get(engine(), streamed_tx())?.clone();
                    let screen_state = scene.get(engine(), screen_state())?.clone();

                    this.open_viewport(
                        &renderer_commands,
                        scene_id,
                        streamed_tx,
                        screen_state,
                        new_listener.clone(),
                    )?;
                }
                this.listeners.push(new_listener);
            }

            for event in rx.drain() {
                let scene_id = event.id;
                tracing::info!(?event.kind, ?event.key);

                match event.kind {
                    flax::events::EventKind::Added => {
                        this.process_new_scene(ctx.world, scene_id)
                            .context("Failed to open viewport for scene")?
                    }
                    flax::events::EventKind::Removed => {
                        this.process_removed_scene(ctx.world, scene_id)
                            .context("Failed to process removed scene")?;
                    }
                    flax::events::EventKind::Modified => {}
                }
            }

            Ok(())
        });

        Ok(())
    }
}
