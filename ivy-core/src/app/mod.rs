mod builder;

pub use builder::*;
use hecs::World;

use crate::layer::{Layer, LayerStack};

pub struct App {
    name: String,
    world: World,
    layers: LayerStack,
}

impl App {
    pub fn new() -> Self {
        Self {
            name: "Ivy".into(),
            layers: LayerStack::new(),
            world: World::new(),
        }
    }

    pub fn builder() -> AppBuilder {
        AppBuilder::new()
    }

    /// Enters the main application event loop and runs the layers.
    pub fn run(&mut self) {
        let world = &mut self.world;
        // Run attach on layers in order
        self.layers
            .iter_mut()
            .for_each(|layer| layer.on_attach(world));

        // Update layers
        loop {
            for layer in self.layers.iter_mut() {
                layer.on_update(world);
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
