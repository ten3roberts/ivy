mod builder;
mod event;

use std::time::Duration;

pub use builder::*;
pub use event::*;

use flume::Receiver;
use hecs::World;

use crate::{
    layer::{Layer, LayerStack},
    Clock, Events, Gizmos, IntoDuration,
};

use ivy_resources::Resources;

pub struct App {
    name: String,

    layers: LayerStack,

    resources: Resources,
    world: World,
    events: Events,

    rx: Receiver<AppEvent>,

    running: bool,

    event_cleanup_time: Duration,
}

impl App {
    pub fn new() -> Self {
        let mut events = Events::new();

        let (tx, rx) = flume::unbounded();
        events.subscribe_custom(tx);
        let resources = Resources::new();

        // Will never fail
        resources.insert(Gizmos::default()).unwrap();

        Self {
            name: "Ivy".into(),
            layers: LayerStack::new(),
            world: World::new(),
            resources,
            events,
            rx,
            running: false,
            event_cleanup_time: 60.0.secs(),
        }
    }

    pub fn builder() -> AppBuilder {
        AppBuilder::new()
    }

    /// Enters the main application event loop and runs the layers.
    pub fn run(&mut self) -> anyhow::Result<()> {
        eprintln!("Running app");
        self.running = true;

        let mut frame_clock = Clock::new();

        let mut event_cleanup = Clock::new();

        // Update layers
        while self.running {
            if event_cleanup.elapsed() > self.event_cleanup_time {
                event_cleanup.reset();
                self.events.cleanup();
            }

            let frame_time = frame_clock.reset();
            let world = &mut self.world;
            let resources = &mut self.resources;
            let events = &mut self.events;

            for layer in self.layers.iter_mut() {
                layer.on_update(world, resources, events, frame_time)?;
            }

            // Read all events sent by application
            self.handle_events();
        }
        Ok(())
    }

    pub fn handle_events(&mut self) {
        for event in self.rx.try_iter() {
            match event {
                AppEvent::Exit => self.running = false,
            }
        }
    }

    /// Return a reference to the application's name.
    pub fn name(&self) -> &String {
        &self.name
    }

    /// Pushes a layer from the provided init closure to to the top of the layer stack. The provided
    /// closure to construct the layer takes in the world and events.
    pub fn push_layer<F, T>(&mut self, func: F)
    where
        F: FnOnce(&mut World, &mut Resources, &mut Events) -> T,
        T: 'static + Layer,
    {
        let layer = func(&mut self.world, &mut self.resources, &mut self.events);
        self.layers.push(layer);
    }

    /// Pushes a layer from the provided init closure to to the top of the layer stack. The provided
    /// closure to construct the layer takes in the world and events, and may return an error which
    /// is propagated to the callee.
    pub fn try_push_layer<F, T, E>(&mut self, func: F) -> Result<(), E>
    where
        F: FnOnce(&mut World, &mut Resources, &mut Events) -> Result<T, E>,
        T: 'static + Layer,
    {
        let layer = func(&mut self.world, &mut self.resources, &mut self.events)?;
        self.layers.push(layer);
        Ok(())
    }

    /// Inserts a layer from the provided init closure to to the top of the layer stack. The provided
    /// closure to construct the layer takes in the world and events.
    pub fn insert_layer<F, T>(&mut self, index: usize, func: F)
    where
        F: FnOnce(&mut World, &mut Resources, &mut Events) -> T,
        T: 'static + Layer,
    {
        let layer = func(&mut self.world, &mut self.resources, &mut self.events);
        self.layers.insert(index, layer);
    }

    /// Pushes a layer from the provided init closure to to the top of the layer stack. The provided
    /// closure to construct the layer takes in the world and events, and may return an error which
    /// is propagated to the callee.
    pub fn try_insert_layer<F, T, E>(&mut self, index: usize, func: F) -> Result<(), E>
    where
        F: FnOnce(&mut World, &mut Resources, &mut Events) -> Result<T, E>,
        T: 'static + Layer,
    {
        let layer = func(&mut self.world, &mut self.resources, &mut self.events)?;
        self.layers.insert(index, layer);
        Ok(())
    }

    /// Get a mutable reference to the app's world.
    pub fn world_mut(&mut self) -> &mut World {
        &mut self.world
    }

    /// Get a mutable reference to the app's events.
    pub fn events_mut(&mut self) -> &mut Events {
        &mut self.events
    }

    /// Get a mutable reference to the app's resources.
    pub fn resources_mut(&mut self) -> &mut Resources {
        &mut self.resources
    }

    /// Get a reference to the app's world.
    pub fn world(&self) -> &World {
        &self.world
    }

    /// Get a reference to the app's events.
    pub fn events(&self) -> &Events {
        &self.events
    }

    /// Get a reference to the app's resources.
    pub fn resources(&self) -> &Resources {
        &self.resources
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
