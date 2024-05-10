mod builder;
pub mod driver;
pub mod event;

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    time::Duration,
};

pub use builder::*;
pub use event::*;

use flax::World;
use flume::Receiver;

use crate::{
    layer::events::{Event, EventDispatcher, EventRegisterContext, EventRegistry},
    Events, Layer, LayerDyn,
};

use ivy_assets::AssetCache;

use self::driver::Driver;

pub struct App {
    name: String,

    layers: Vec<Box<dyn LayerDyn>>,
    /// Event bus for layers
    pub event_registry: EventRegistry,

    pub assets: AssetCache,
    pub world: World,
    #[deprecated(note = "Use ECS instead")]
    pub events: Events,

    rx: Receiver<AppEvent>,

    running: bool,
}

impl App {
    pub fn new() -> Self {
        let mut events = Events::new();

        let (tx, rx) = flume::unbounded();
        events.subscribe_custom(tx);
        let asset_cache = AssetCache::new();

        // asset_cache.insert(Gizmos::default());

        Self {
            name: "Ivy".into(),
            layers: Default::default(),
            event_registry: Default::default(),
            world: World::new(),
            assets: asset_cache,
            events,
            rx,
            running: false,
        }
    }

    pub fn builder() -> AppBuilder {
        AppBuilder::new()
    }

    pub fn tick(&mut self, delta: Duration) -> anyhow::Result<()> {
        self.event_registry.emit(
            &mut self.layers,
            &mut self.world,
            &mut self.assets,
            &TickEvent,
        )
    }

    pub fn init(&mut self) -> anyhow::Result<()> {
        for (index, layer) in &mut self.layers.iter_mut().enumerate() {
            layer.register_dyn(
                &mut self.world,
                &self.assets,
                &mut self.event_registry,
                index,
            )?;
        }

        self.event_registry.emit(
            &mut self.layers,
            &mut self.world,
            &mut self.assets,
            &InitEvent,
        )
    }

    pub fn run(&mut self, driver: &mut (impl Driver + ?Sized)) -> anyhow::Result<()> {
        driver.enter(self)
    }

    /// Return a reference to the application's name.
    pub fn name(&self) -> &String {
        &self.name
    }

    pub fn push_layer<T: Layer>(&mut self, layer: T) {
        self.layers.push(Box::new(layer));
    }

    /// Get a mutable reference to the app's world.
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Get a mutable reference to the app's events.
    pub fn events_mut(&mut self) -> &mut Events {
        &mut self.events
    }

    /// Get a mutable reference to the app's asset_cache.
    pub fn asset_cache_mut(&mut self) -> &mut AssetCache {
        &mut self.assets
    }

    /// Get a reference to the app's world.
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Emits an event to all layers.
    pub fn emit<T: Event>(&mut self, event: T) -> anyhow::Result<()> {
        self.event_registry
            .emit(&mut self.layers, &mut self.world, &mut self.assets, &event)
    }

    /// Get a reference to the app's events.
    pub fn events(&self) -> &Events {
        &self.events
    }

    /// Get a reference to the app's asset_cache.
    pub fn asset_cache(&self) -> &AssetCache {
        &self.assets
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
