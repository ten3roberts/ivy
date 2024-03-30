use flax::{component::ComponentValue, entity_ids, Component, EntityRef, Query, World};

pub trait WorldExt {
    /// Finds an entity by name
    fn by_name(&self, name: &str) -> Option<EntityRef>;
    /// Finds an entity by tag
    fn by_tag<T: ComponentValue>(&self, component: Component<T>) -> Option<EntityRef>;
}

impl WorldExt for World {
    fn by_name(&self, name: &str) -> Option<EntityRef> {
        Query::new((entity_ids(), flax::components::name()))
            .borrow(self)
            .iter()
            .find(|(_, val)| **val == name)
            .map(|(v, _)| self.entity(v).unwrap())
    }

    fn by_tag<T: ComponentValue>(&self, component: Component<T>) -> Option<EntityRef> {
        Query::new((entity_ids(), component))
            .borrow(self)
            .iter()
            .next()
            .map(|(v, _)| self.entity(v).unwrap())
    }
}
