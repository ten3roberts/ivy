mod builder;

pub use builder::*;

use crate::layer::{Layer, LayerStack};

pub struct App {
    name: String,
    layers: LayerStack,
}

impl App {
    pub fn new() -> Self {
        Self {
            name: "Ivy".into(),
            layers: LayerStack::new(),
        }
    }

    pub fn builder() -> AppBuilder {
        AppBuilder::new()
    }

    pub fn run(&mut self) {
        self.layers.iter_mut().for_each(|layer| layer.on_attach());

        loop {
            for layer in self.layers.iter_mut() {
                layer.on_update();
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
