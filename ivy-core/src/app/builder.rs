use super::*;
pub struct AppBuilder {
    application: App,
}

impl AppBuilder {
    pub fn new() -> Self {
        Self {
            application: App::new(),
        }
    }

    pub fn build(&mut self) -> App {
        std::mem::replace(&mut self.application, App::new())
    }

    pub fn push_layer<T: 'static + Layer>(&mut self, layer: T) -> &mut Self {
        self.application.push_layer(layer);
        self
    }
}
