use super::*;
pub struct AppBuilder {
    app: App,
}

impl AppBuilder {
    pub fn new() -> Self {
        Self { app: App::new() }
    }

    pub fn build(&mut self) -> App {
        std::mem::replace(&mut self.app, App::new())
    }
    ///
    /// Pushes a layer from the provided init closure to to the top of the layer stack. The provided
    /// closure to construct the layer takes in the world and events.
    pub fn push_layer<F, T>(&mut self, func: F) -> &mut Self
    where
        F: FnOnce(&mut World, &mut Events) -> T,
        T: 'static + Layer,
    {
        self.app.push_layer(func);
        self
    }

    /// Pushes a layer from the provided init closure to to the top of the layer stack. The provided
    /// closure to construct the layer takes in the world and events, and may return an error which
    /// is propagated to the callee.
    pub fn try_push_layer<F, T, E>(&mut self, func: F) -> Result<&mut Self, E>
    where
        F: FnOnce(&mut World, &mut Events) -> Result<T, E>,
        T: 'static + Layer,
    {
        self.app.try_push_layer(func)?;
        Ok(self)
    }
}

impl Default for AppBuilder {
    fn default() -> Self {
        Self::new()
    }
}
