mod builder;

pub use builder::*;
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
}

impl App {
    pub fn new() -> Self {
        Self {
            name: "Ivy".into(),
            layers: LayerStack::new(),
            world: World::new(),
            events: Events::new(),
        }
    }

    pub fn builder() -> AppBuilder {
        AppBuilder::new()
    }

    /// Enters the main application event loop and runs the layers.
    pub fn run(&mut self) {
        let world = &mut self.world;
        let events = &mut self.events;
        // Run attach on layers in order
        self.layers
            .iter_mut()
            .for_each(|layer| layer.on_attach(world, events));

        // Update layers
        loop {
            for layer in self.layers.iter_mut() {
                layer.on_update(world, events);
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
