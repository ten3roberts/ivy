use flax::EntityBuilder;
pub use ivy_derive::Bundle;

/// A bundle describes a set of related components mounted to an entity.
pub trait Bundle: Send + Sync {
    fn mount(&self, entity: &mut EntityBuilder);
}

pub trait EntityBuilderExt {
    fn mount<T: Bundle>(&mut self, bundle: T) -> &mut Self;
}

impl EntityBuilderExt for EntityBuilder {
    fn mount<T: Bundle>(&mut self, bundle: T) -> &mut Self {
        bundle.mount(self);
        self
    }
}
