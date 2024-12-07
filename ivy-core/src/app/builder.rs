use self::driver::{DefaultDriver, Driver};
use super::*;

pub struct AppBuilder {
    app: App,
    driver: Box<dyn Driver>,
}

impl AppBuilder {
    pub fn new() -> Self {
        Self {
            app: App::new(),
            driver: Box::new(DefaultDriver {}),
        }
    }

    /// Set the name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.app.name = name.into();
        self
    }

    pub fn build(self) -> App {
        self.app
    }

    pub fn run(mut self) -> anyhow::Result<()> {
        self.app.run(&mut *self.driver)
    }

    pub fn with_driver(mut self, driver: impl 'static + Driver) -> Self {
        self.driver = Box::new(driver);
        self
    }

    /// Pushes a layer from the provided init closure to to the top of the layer stack. The provided
    /// closure to construct the layer takes in the world and events.
    pub fn with_layer<T: Layer>(mut self, layer: T) -> Self {
        self.app.push_layer(layer);
        self
    }
}

impl Default for AppBuilder {
    fn default() -> Self {
        Self::new()
    }
}
