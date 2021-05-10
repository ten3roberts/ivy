use crate::layer::{Layer, LayerStack};

pub struct ApplicationBuilder {
    application: Application,
}

impl ApplicationBuilder {
    pub fn new() -> Self {
        Self {
            application: Application::new(),
        }
    }

    pub fn build(&mut self) -> Application {
        std::mem::replace(&mut self.application, Application::new())
    }

    pub fn push_layer<T: 'static + Layer>(&mut self, layer: T) -> &mut Self {
        self.application.push_layer(layer);
        self
    }
}

pub struct Application {
    name: String,
    layers: LayerStack,
}

impl Application {
    pub fn new() -> Self {
        Self {
            name: "Ivy".into(),
            layers: LayerStack::new(),
        }
    }

    pub fn builder() -> ApplicationBuilder {
        ApplicationBuilder::new()
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
