use super::*;
use ivy_resources::Resources;

pub struct AppBuilder {
    app: App,
}

impl AppBuilder {
    pub fn new() -> Self {
        Self { app: App::new() }
    }

    pub fn build(self) -> App {
        self.app
    }

    /// Pushes a layer from the provided init closure to to the top of the layer stack. The provided
    /// closure to construct the layer takes in the world and events.
    pub fn push_layer<F, T>(mut self, func: F) -> Self
    where
        F: FnOnce(&mut World, &mut Resources, &mut Events) -> T,
        T: 'static + Layer,
    {
        self.app.push_layer(func);
        self
    }

    /// Pushes a layer from the provided init closure to to the top of the layer stack. The provided
    /// closure to construct the layer takes in the world and events, and may return an error which
    /// is propagated to the callee.
    pub fn try_push_layer<F, T, E>(mut self, func: F) -> Result<Self, E>
    where
        F: FnOnce(&mut World, &mut Resources, &mut Events) -> Result<T, E>,
        T: 'static + Layer,
    {
        self.app.try_push_layer(func)?;
        Ok(self)
    }

    /// Inserts a layer from the provided init closure to to the top of the layer stack. The provided
    /// closure to construct the layer takes in the world and events.
    pub fn insert_layer<F, T>(mut self, index: usize, func: F) -> Self
    where
        F: FnOnce(&mut World, &mut Resources, &mut Events) -> T,
        T: 'static + Layer,
    {
        self.app.insert_layer(index, func);
        self
    }

    /// Inserts a layer from the provided init closure to to the top of the layer stack. The provided
    /// closure to construct the layer takes in the world and events, and may return an error which
    /// is propagated to the callee.
    pub fn try_insert_layer<F, T, E>(mut self, index: usize, func: F) -> Result<Self, E>
    where
        F: FnOnce(&mut World, &mut Resources, &mut Events) -> Result<T, E>,
        T: 'static + Layer,
    {
        self.app.try_insert_layer(index, func)?;
        Ok(self)
    }
}

impl Default for AppBuilder {
    fn default() -> Self {
        Self::new()
    }
}
