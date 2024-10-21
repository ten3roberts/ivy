mod builder;
pub mod driver;
pub mod event;

use std::time::Duration;

pub use builder::*;
pub use event::*;

use flax::World;

use crate::{
    components::{self, engine},
    layer::events::{Event, EventRegistry},
    Layer, LayerDyn,
};

use ivy_assets::{service::FileSystemMapService, AssetCache};

use self::driver::Driver;

pub struct App {
    name: String,

    layers: Vec<Box<dyn LayerDyn>>,
    /// Event bus for layers
    pub event_registry: EventRegistry,

    pub assets: AssetCache,
    pub world: World,

    running: bool,
}

impl App {
    pub fn new() -> Self {
        let asset_cache = AssetCache::new();
        asset_cache.register_service(FileSystemMapService::new("./assets"));

        let mut world = World::new();
        world
            .set(engine(), components::gizmos(), Default::default())
            .unwrap();

        #[allow(deprecated)]
        Self {
            name: "Ivy".into(),
            layers: Default::default(),
            event_registry: Default::default(),
            world,
            assets: asset_cache,
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
            &TickEvent(delta),
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
