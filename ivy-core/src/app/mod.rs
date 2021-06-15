mod builder;
mod event;

pub use builder::*;
pub use event::*;

use flume::Receiver;
use hecs::World;

use crate::{
    layer::{Layer, LayerStack},
    Clock, Events,
};

pub struct App {
    name: String,

    layers: LayerStack,

    world: World,
    events: Events,

    rx: Receiver<AppEvent>,

    running: bool,
}

impl App {
    pub fn new() -> Self {
        let mut events = Events::new();

        let (tx, rx) = flume::unbounded();
        events.subscribe(tx);

        Self {
            name: "Ivy".into(),
            layers: LayerStack::new(),
            world: World::new(),
            events,
            rx,
            running: false,
        }
    }

    pub fn builder() -> AppBuilder {
        AppBuilder::new()
    }

    /// Enters the main application event loop and runs the layers.
    pub fn run(&mut self) -> anyhow::Result<()> {
        self.running = true;

        let mut frame_clock = Clock::new();

        // Update layers
        while self.running {
            let frame_time = frame_clock.reset();
            let world = &mut self.world;
            let events = &mut self.events;

            for layer in self.layers.iter_mut() {
                layer.on_update(world, events, frame_time)?;
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
        F: FnOnce(&mut World, &mut Events) -> T,
        T: 'static + Layer,
    {
        let layer = func(&mut self.world, &mut self.events);
        self.layers.push(layer);
    }

    /// Pushes a layer from the provided init closure to to the top of the layer stack. The provided
    /// closure to construct the layer takes in the world and events, and may return an error which
    /// is propagated to the callee.
    pub fn try_push_layer<F, T, E>(&mut self, func: F) -> Result<(), E>
    where
        F: FnOnce(&mut World, &mut Events) -> Result<T, E>,
        T: 'static + Layer,
    {
        let layer = func(&mut self.world, &mut self.events)?;
        self.layers.push(layer);
        Ok(())
    }
}

impl Default for App {
    fn default() -> Self {
        Self::new()
    }
}
