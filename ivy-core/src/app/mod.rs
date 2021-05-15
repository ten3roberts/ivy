mod builder;
mod event;

pub use builder::*;
pub use event::*;

use flume::Receiver;
use hecs::World;

use crate::{
    layer::{Layer, LayerStack},
    Events,
};

pub struct App {
    name: String,
    world: World,
    events: Events,
    layers: LayerStack,

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
    pub fn run(&mut self) {
        self.running = true;

        self.init();

        // Update layers
        while self.running {
            let world = &mut self.world;
            let events = &mut self.events;

            for layer in self.layers.iter_mut() {
                layer.on_update(world, events);
            }

            // Read all events sent by application
            self.handle_events();
        }
    }

    pub fn init(&mut self) {
        let world = &mut self.world;
        let events = &mut self.events;

        // Run attach on layers in order
        self.layers
            .iter_mut()
            .for_each(|layer| layer.on_attach(world, events));
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

    /// Pushes a layer to the end of the layer stack.
    pub fn push_layer<T: 'static + Layer>(&mut self, layer: T) {
        self.layers.push(layer);
    }
}
