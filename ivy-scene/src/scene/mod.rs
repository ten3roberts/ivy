//! Core scene layer.
//!
//! Allows managing multiple game worlds.
pub mod render;
pub mod ser;
pub mod ui;
pub mod viewport_provider;

use std::{ops::DerefMut, sync::Arc};

use flax::{component, Entity, World};
use futures::channel;
use ivy_assets::{stored::DynamicStore, AssetCache};
use ivy_core::{
    app::{PostInitEvent, TickEvent},
    components::engine,
    events::{Event, EventContext, EventRegisterContext, EventRegistry},
    Layer, LayerDyn, WorldExt,
};
use ivy_wgpu::{
    components::{main_window, viewport_size, window},
    events::{ApplicationReady, WindowResizedEvent},
    types::Window,
};

pub struct OpenSceneCommand {
    builder: Box<dyn Send + FnOnce() -> SceneBuilder>,
    on_ready: Option<channel::oneshot::Sender<Entity>>,
}

impl OpenSceneCommand {
    pub fn new(
        builder: impl Send + 'static + FnOnce() -> SceneBuilder,
        on_ready: Option<channel::oneshot::Sender<Entity>>,
    ) -> Self {
        Self {
            builder: Box::new(builder),
            on_ready,
        }
    }
}

pub enum SceneCommand {
    CloseScene,
    OpenScene(OpenSceneCommand),
}

impl SceneCommand {}

component! {
    pub scene_commands: flume::Sender<SceneCommand>,
    pub scene_world: World,
    pub on_new_scene: Vec<Box<dyn Send + Sync + FnMut(&World, Entity) -> bool>>,
}

/// A scene represents a world within the ivy engine
///
/// A scene can be loaded and unloaded at any time
pub struct Scene {
    world_id: Entity,
    layers: Vec<Box<dyn LayerDyn>>,
    event_registry: EventRegistry,
}

impl Scene {
    pub fn builder() -> SceneBuilder {
        SceneBuilder {
            world: World::new(),
            layers: Vec::new(),
        }
    }

    pub fn new(world_id: Entity) -> Self {
        Self {
            world_id,
            layers: Vec::new(),
            event_registry: Default::default(),
        }
    }

    fn sync_engine_components(&mut self, engine_world: &World) -> anyhow::Result<()> {
        let _engine_entity = engine_world.entity(engine()).expect("Engine present");

        let world = &mut *engine_world.get_mut(self.world_id, scene_world())?;

        if let Some(window_entity) = engine_world.by_tag(main_window()) {
            Entity::builder()
                .set_default(main_window())
                .set(window(), window_entity.get(window())?.clone())
                .spawn(world);
        }

        Entity::builder()
            .set_default(viewport_size())
            // .set_opt(audio_mixer(), engine_entity.get_clone(audio_mixer()).ok())
            .append_to(world, engine())?;

        Ok(())
    }

    fn get_world<'a>(
        &self,
        engine_world: &'a mut World,
    ) -> flax::error::Result<impl DerefMut<Target = World> + 'a> {
        engine_world.get_mut(self.world_id, scene_world())
    }

    pub fn world_id(&self) -> Entity {
        self.world_id
    }

    pub fn emit_event<T: Event>(
        &mut self,
        engine_world: &mut World,
        assets: &AssetCache,
        store: &mut DynamicStore,
        event: &T,
    ) -> anyhow::Result<()> {
        let world = &mut *self.get_world(engine_world)?;

        self.event_registry.emit(
            &mut self.layers,
            &mut EventContext {
                world,
                assets,
                store,
            },
            event,
        )
    }

    pub fn emit_event_dyn(
        &mut self,
        engine_world: &mut World,
        assets: &AssetCache,
        store: &mut DynamicStore,
        event: &dyn Event,
    ) -> anyhow::Result<bool> {
        let world = &mut *self.get_world(engine_world)?;

        self.event_registry.emit_dyn(
            &mut self.layers,
            &mut EventContext {
                world,
                assets,
                store,
            },
            event,
        )
    }
}

/// Allows nesting a scene within the engine
pub struct SceneLayer {
    active_scene: Option<Scene>,
    staged_scene: Option<SceneBuilder>,
    scene_command_rx: flume::Receiver<SceneCommand>,
    scene_commands_tx: flume::Sender<SceneCommand>,
    // TODO: don't use events for this
    window: Option<Arc<Window>>,
}

impl SceneLayer {
    pub fn new() -> Self {
        let (tx, rx) = flume::unbounded();

        Self {
            active_scene: None,
            staged_scene: None,
            scene_command_rx: rx,
            scene_commands_tx: tx,
            window: None,
        }
    }

    pub fn with_scene(mut self, scene: SceneBuilder) -> Self {
        self.staged_scene = Some(scene);
        self
    }

    fn process_commands(
        &mut self,
        engine_world: &mut World,
        assets: &AssetCache,
        store: &mut DynamicStore,
    ) -> anyhow::Result<()> {
        if let Some(staged) = self.staged_scene.take() {
            if let Some(old_scene) = self.active_scene.take() {
                engine_world.despawn(old_scene.world_id)?;
            }

            let new_scene = self.process_staging_scene(engine_world, assets, store, staged)?;

            self.active_scene = Some(new_scene);
        }

        for cmd in self.scene_command_rx.drain() {
            match cmd {
                SceneCommand::OpenScene(cmd) => {
                    if let Some(old_scene) = self.active_scene.take() {
                        engine_world.despawn(old_scene.world_id)?;
                    }

                    let new_scene =
                        self.process_staging_scene(engine_world, assets, store, (cmd.builder)())?;

                    if let Some(on_ready) = cmd.on_ready {
                        let _ = on_ready.send(new_scene.world_id);
                    }

                    self.active_scene = Some(new_scene);
                }
                SceneCommand::CloseScene => {
                    if let Some(old_scene) = self.active_scene.take() {
                        engine_world.despawn(old_scene.world_id)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn process_staging_scene(
        &self,
        engine_world: &mut World,
        assets: &AssetCache,
        store: &mut DynamicStore,
        mut scene: SceneBuilder,
    ) -> anyhow::Result<Scene> {
        let mut event_registry = EventRegistry::new();

        for (index, layer) in &mut scene.layers.iter_mut().enumerate() {
            layer.register_dyn(&mut scene.world, assets, store, &mut event_registry, index)?;
        }

        let world_id = Entity::builder()
            .set(scene_world(), scene.world)
            .spawn(engine_world);

        let mut scene = Scene {
            world_id,
            layers: scene.layers,
            event_registry,
        };

        scene.sync_engine_components(engine_world)?;

        scene
            .get_world(engine_world)?
            .set(engine(), scene_commands(), self.scene_commands_tx.clone())
            .unwrap();

        scene.emit_event(engine_world, assets, store, &PostInitEvent)?;

        if let Some(window) = &self.window {
            scene.emit_event(
                engine_world,
                assets,
                store,
                &ApplicationReady(window.clone()),
            )?;

            scene.emit_event(
                engine_world,
                assets,
                store,
                &WindowResizedEvent {
                    physical_size: window.inner_size(),
                    logical_size: window.inner_size().to_logical(window.scale_factor()),
                },
            )?;
        }

        Ok(scene)
    }
}

impl Default for SceneLayer {
    fn default() -> Self {
        Self::new()
    }
}

pub struct SceneBuilder {
    world: World,
    layers: Vec<Box<dyn LayerDyn>>,
}

impl SceneBuilder {
    /// Set the world
    pub fn with_world(mut self, world: World) -> Self {
        self.world = world;
        self
    }

    pub fn with_layer<L: Layer>(mut self, layer: L) -> Self {
        self.layers.push(Box::new(layer));
        self
    }
}

impl Layer for SceneLayer {
    fn register(
        &mut self,
        world: &mut flax::World,
        _: &AssetCache,
        _store: &mut DynamicStore,
        mut events: EventRegisterContext<Self>,
    ) -> anyhow::Result<()>
    where
        Self: Sized,
    {
        world.set(engine(), scene_commands(), self.scene_commands_tx.clone())?;

        events.subscribe_global(|this, ctx, event| {
            let Some(scene) = &mut this.active_scene else {
                return Ok(false);
            };

            scene.emit_event_dyn(ctx.world, ctx.assets, ctx.store, event)
        });

        events.subscribe(|this, ctx, _: &TickEvent| {
            this.process_commands(ctx.world, ctx.assets, ctx.store)?;

            Ok(())
        });

        // TODO: find a better way to share the window around
        //
        // Maybe we don't even need it
        events.subscribe(|this, _, ApplicationReady(window): &ApplicationReady| {
            this.window = Some(window.clone());
            Ok(())
        });

        Ok(())
    }
}
