use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use anyhow::Context;
use flax::{
    events::{EventKindFilter, EventSubscriber},
    Entity, World,
};
use futures::channel::oneshot;
use glam::Vec2;
use ivy_core::{app::TickEvent, components::engine, events::EventContext, Layer};
use ivy_ui::{
    components::ui_instance,
    screens::{screen_state, ScreenState},
    streamed::{streamed_state, StreamedState},
    violet::core::to_owned,
};
use ivy_wgpu::{
    components::viewport_size,
    layer::{gpu_instance, render_graph_handle},
    rendergraph::{
        ManagedTextureDesc, NodeId, RenderGraph, RenderGraphImageDesc, RenderGraphResources,
        TextureHandle,
    },
};
use parking_lot::Mutex;
use wgpu::{TextureDimension, TextureFormat};

use crate::{
    render::SceneRenderNode,
    scene_world,
    ui::{OpenViewport, SceneViewCommand},
};

flax::component! {
    /// Allows listening to and receiving new viewports from the scene system
    pub scene_viewport_state: SceneViewportState,
}

struct ViewportResize {
    scene_id: Entity,
    new_size: Vec2,
    texture_desc: ManagedTextureDesc,
    texture_handle: TextureHandle,
}

#[derive(Clone)]
pub struct SceneViewportState {
    inner: Arc<Mutex<SceneViewportStateInner>>,
}
impl SceneViewportState {
    /// Registers a new listener to receive viewports for all open scenes
    pub fn register_listener(&self) -> flume::Receiver<SceneViewCommand> {
        let (tx, rx) = flume::unbounded();
        self.inner.lock().new_listeners.push(tx);
        rx
    }

    pub fn open_viewport(&self, scene: Entity) -> oneshot::Receiver<OpenViewport> {
        let (tx, rx) = oneshot::channel();
        self.inner.lock().viewport_requests.push((scene, tx));
        rx
    }
}

struct SceneViewportStateInner {
    new_listeners: Vec<flume::Sender<SceneViewCommand>>,
    viewport_requests: Vec<(Entity, oneshot::Sender<OpenViewport>)>,
}

/// Hooks into a new scene and provides it with a viewport
pub struct SceneViewportProvider {
    open_scenes: BTreeSet<Entity>,
    state: SceneViewportState,
    listeners: Vec<flume::Sender<SceneViewCommand>>,
    pending_resizes_rx: flume::Receiver<ViewportResize>,
    pending_resizes_tx: flume::Sender<ViewportResize>,
    proxy_nodes:
        Arc<Mutex<BTreeMap<Entity, Vec<(NodeId, TextureHandle, flume::Sender<SceneViewCommand>)>>>>,
}

impl SceneViewportProvider {
    pub fn new() -> Self {
        let (pending_resizes_tx, pending_resizes_rx) = flume::unbounded();

        Self {
            open_scenes: BTreeSet::new(),
            state: SceneViewportState {
                inner: Arc::new(Mutex::new(SceneViewportStateInner {
                    new_listeners: Vec::new(),
                    viewport_requests: Vec::new(),
                })),
            },
            listeners: Vec::new(),
            proxy_nodes: Arc::new(Mutex::new(BTreeMap::new())),
            pending_resizes_rx,
            pending_resizes_tx,
        }
    }
}

impl Layer for SceneViewportProvider {
    fn register(
        &mut self,
        world: &mut World,
        _assets: &ivy_assets::AssetCache,
        store: &mut ivy_assets::stored::DynamicStore,
        mut events: ivy_core::events::EventRegisterContext<Self>,
    ) -> anyhow::Result<()> {
        let (tx, rx) = flume::unbounded();

        world.set(engine(), scene_viewport_state(), self.state.clone())?;

        let mut ui = store.get_mut(&*world.get(engine(), ui_instance())?);
        ui.root_scope()
            .set_context(scene_viewport_state(), self.state.clone());

        world.subscribe(
            tx.filter_arch(scene_world().with())
                .filter_components([scene_world().key()])
                .filter_event_kind(EventKindFilter::ADDED | EventKindFilter::REMOVED),
        );

        events.subscribe(move |this, ctx, _: &TickEvent| {
            {
                let mut inner = this.state.inner.lock();
                for new_listener in inner.new_listeners.drain(..) {
                    for &scene_id in &this.open_scenes {
                        let scene = ctx.world.get_mut(scene_id, scene_world())?;
                        let streamed_tx = scene.get(engine(), streamed_state())?.clone();
                        let screen_state = scene.get(engine(), screen_state())?.clone();
                        drop(scene);

                        this.open_viewport(
                            ctx,
                            scene_id,
                            streamed_tx,
                            screen_state,
                            Some(new_listener.clone()),
                        )?;
                    }
                    this.listeners.push(new_listener);
                }

                for request in inner.viewport_requests.drain(..) {
                    let scene = ctx
                        .world
                        .get_mut(request.0, scene_world())
                        .context("Attempt to open viewport for nonexistent scene")?;

                    let streamed_tx = scene.get(engine(), streamed_state())?.clone();
                    let screen_state = scene.get(engine(), screen_state())?.clone();
                    drop(scene);

                    let result =
                        this.open_viewport(ctx, request.0, streamed_tx, screen_state, None)?;
                    let _ = request.1.send(result);
                }
            }

            for mut resize in this.pending_resizes_rx.drain() {
                let render_graph_handle = ctx.world.get(engine(), render_graph_handle())?.clone();
                let mut render_graph = ctx.store.get_mut(&render_graph_handle);

                let scene = ctx.world.get(resize.scene_id, scene_world())?;
                scene.update_dedup(engine(), viewport_size(), resize.new_size)?;

                let texture = render_graph
                    .resources
                    .get_texture_mut(resize.texture_handle);

                resize.texture_desc.extent = wgpu::Extent3d {
                    width: resize.new_size.x as u32,
                    height: resize.new_size.y as u32,
                    depth_or_array_layers: 1,
                };

                if let RenderGraphImageDesc::Managed(managed) = texture {
                    *managed = resize.texture_desc;
                } else {
                    tracing::warn!("Tried to resize a non-managed texture");
                }
            }

            for event in rx.drain() {
                let scene_id = event.id;
                tracing::info!(?event.kind, ?event.key);

                match event.kind {
                    flax::events::EventKind::Added => {
                        tracing::info!(?scene_id, "Scene opened");
                        this.process_new_scene(ctx, scene_id)
                            .context("Failed to open viewport for scene")?
                    }
                    flax::events::EventKind::Removed => {
                        tracing::info!(?scene_id, "Scene closed");
                        this.process_removed_scene(ctx, scene_id)
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

impl SceneViewportProvider {
    fn process_new_scene(
        &mut self,
        ctx: &mut EventContext,
        scene_id: Entity,
    ) -> anyhow::Result<()> {
        let proxy_nodes = self.proxy_nodes.clone();

        if proxy_nodes.lock().contains_key(&scene_id) {
            tracing::info!("Duplicate scene viewport request for scene {scene_id:?}");
            return Ok(());
        }

        self.open_scenes.insert(scene_id);

        let scene = ctx.world.get_mut(scene_id, scene_world())?;
        let streamed_tx = scene.get(engine(), streamed_state())?.clone();
        let screen_state = scene.get(engine(), screen_state())?.clone();
        drop(scene);

        for tx in self.listeners.iter().cloned() {
            to_owned!(scene_id, streamed_tx, screen_state);

            let result =
                self.open_viewport(ctx, scene_id, streamed_tx, screen_state, Some(tx.clone()))?;
            let _ = tx.send(SceneViewCommand::OpenViewport(result));
        }

        Ok(())
    }

    fn process_removed_scene(
        &mut self,
        ctx: &mut ivy_core::events::EventContext,
        scene_id: Entity,
    ) -> anyhow::Result<()> {
        let render_graph = ctx.world.get(engine(), render_graph_handle())?.clone();
        let mut render_graph = ctx.store.get_mut(&render_graph);

        self.open_scenes.remove(&scene_id);

        let nodes = self
            .proxy_nodes
            .lock()
            .remove(&scene_id)
            .context("Scene was never added")?;

        // Remove all nodes associated to the old scene for all open viewports
        for (node_id, texture_handle, tx) in nodes {
            // TODO: thread local storage for non-callback access
            render_graph
                .remove_node(node_id)
                .context("Missing proxy node")?;

            render_graph
                .resources
                .remove_texture(texture_handle)
                .context("Missing texture for proxy node")?;

            let _ = tx.send(SceneViewCommand::CloseViewport { scene: scene_id });
        }

        Ok(())
    }

    fn open_viewport(
        &self,
        ctx: &mut ivy_core::events::EventContext,
        scene_id: Entity,
        streamed: StreamedState,
        screen_state: ScreenState,
        tx: Option<flume::Sender<SceneViewCommand>>,
    ) -> Result<OpenViewport, anyhow::Error> {
        let proxy_nodes = self.proxy_nodes.clone();
        let render_graph_handle = ctx.world.get(engine(), render_graph_handle())?.clone();
        let gpu = ctx.world.get(engine(), gpu_instance())?.clone();

        let mut render_graph = ctx.store.get_mut(&render_graph_handle);
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

        drop(render_graph);
        let node = SceneRenderNode::new(
            ctx.world,
            &gpu,
            ctx.assets,
            ctx.store,
            scene_id,
            subgraph,
            scene_render,
        );

        let render_graph = &mut *ctx.store.get_mut(&render_graph_handle);
        let node_id = render_graph.add_node(node);

        if let Some(tx) = tx {
            proxy_nodes
                .lock()
                .entry(scene_id)
                .or_default()
                .push((node_id, scene_render, tx));
        }

        let pending_resizes = self.pending_resizes_tx.clone();
        let on_size = move |size: Vec2| {
            let px = size.as_uvec2();

            scene_render_desc.extent.width = px.x;
            scene_render_desc.extent.height = px.y;
            let _ = pending_resizes.send(ViewportResize {
                scene_id,
                new_size: size,
                texture_desc: scene_render_desc.clone(),
                texture_handle: scene_render,
            });
        };

        Ok(OpenViewport {
            scene: scene_id,
            view: scene_render,
            on_size: Box::new(on_size),
            streamed,
            screen_state,
        })
    }
}
