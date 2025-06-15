use flax::{Entity, EntityBuilder};
use ivy_core::Bundle;

/// Defines an entity template to construct an entity using [[Bundle]]s
pub struct Template {
    bundles: Vec<Box<dyn Send + Sync + Bundle>>,
}

impl Template {
    pub fn new() -> Self {
        Self {
            bundles: Vec::new(),
        }
    }

    pub fn with_bundle<B: 'static + Send + Sync + Bundle>(mut self, bundle: B) -> Self {
        self.bundles.push(Box::new(bundle));
        self
    }

    pub fn build(&self) -> EntityBuilder {
        let mut entity = Entity::builder();
        for bundle in &self.bundles {
            bundle.mount(&mut entity);
        }
        entity
    }
}
