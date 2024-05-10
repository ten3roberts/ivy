use std::{
    any::{Any, TypeId},
    collections::HashMap,
};

use crate::Layer;

/// Abstracts the stack of layered execution logic
pub struct LayerStack {
    layers: Vec<Box<dyn Any>>,
    event_dispatchers: HashMap<TypeId, Box<dyn Any>>,
}

impl LayerStack {
    pub fn new() -> Self {
        Self {
            layers: Vec::new(),
            event_dispatchers: HashMap::new(),
        }
    }

    pub fn iter(&self) -> std::slice::Iter<Box<dyn Any>> {
        self.layers.iter()
    }

    pub fn iter_mut(&mut self) -> std::slice::IterMut<Box<dyn Any>> {
        self.layers.iter_mut()
    }

    pub fn push<T: 'static + Layer>(&mut self, layer: T) {
        let layer = Box::new(layer);
        self.layers.push(layer);
    }

    pub fn insert<T: 'static + Layer>(&mut self, index: usize, layer: T) {
        let layer = Box::new(layer);
        self.layers.insert(index, layer);
    }
}

impl Default for LayerStack {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> IntoIterator for &'a LayerStack {
    type Item = &'a Box<dyn Any>;

    type IntoIter = std::slice::Iter<'a, Box<dyn Any>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

impl<'a> IntoIterator for &'a mut LayerStack {
    type Item = &'a mut Box<dyn Any>;

    type IntoIter = std::slice::IterMut<'a, Box<dyn Any>>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter_mut()
    }
}
