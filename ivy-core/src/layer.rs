use hecs::World;

/// A layer represents an ordered abstraction of execution logic. Layers are ordered and run in
/// order.
pub trait Layer {
    /// Called for each iteration of the application event loop
    fn on_update(&mut self, world: &mut World);
    /// Called when a layer is attached, I.e, on application startup
    fn on_attach(&mut self, world: &mut World);
}

/// Abstracts the stack of layered execution logic
pub struct LayerStack {
    layers: Vec<Box<dyn Layer>>,
}

impl LayerStack {
    pub fn new() -> Self {
        Self { layers: Vec::new() }
    }

    pub fn iter<'a>(&'a self) -> std::slice::Iter<'a, Box<dyn Layer>> {
        self.layers.iter()
    }

    pub fn iter_mut<'a>(&'a mut self) -> std::slice::IterMut<'a, Box<dyn Layer>> {
        self.layers.iter_mut()
    }

    pub fn push<T: 'static + Layer>(&mut self, layer: T) {
        let layer = Box::new(layer);
        self.layers.push(layer);
    }
}

impl<'a> IntoIterator for &'a LayerStack {
    type Item = &'a Box<dyn Layer>;

    type IntoIter = std::slice::Iter<'a, Box<dyn Layer>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut LayerStack {
    type Item = &'a mut Box<dyn Layer>;

    type IntoIter = std::slice::IterMut<'a, Box<dyn Layer>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
