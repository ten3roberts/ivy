use std::sync::Arc;

use flax::{
    component::ComponentValue, components::child_of, entity_ids, CommandBuffer, Component, Entity,
    EntityBuilder, EntityRef, Query, World,
};
use parking_lot::{Mutex, MutexGuard};

pub trait WorldExt {
    /// Finds an entity by name
    fn by_name(&self, name: &str) -> Option<EntityRef>;
    /// Finds an entity by tag
    fn by_tag<T: ComponentValue>(&self, component: Component<T>) -> Option<EntityRef>;

    fn append_all<I: IntoIterator<Item = (Entity, T)>, T: ComponentValue>(
        &mut self,
        component: Component<T>,
        iter: I,
    ) -> flax::error::Result<()>;

    fn root_entity(&self, entity: Entity) -> EntityRef;
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

    fn append_all<I: IntoIterator<Item = (Entity, T)>, T: ComponentValue>(
        &mut self,
        component: Component<T>,
        iter: I,
    ) -> flax::error::Result<()> {
        iter.into_iter()
            .try_for_each(|(id, value)| self.set(id, component, value).map(|_| {}))
    }

    fn root_entity(&self, id: Entity) -> EntityRef {
        let mut entity = self.entity(id).expect("invalid entity");
        while let Some((parent, _)) = entity.relations(child_of).next() {
            entity = self.entity(parent).expect("dead parent");
        }

        entity
    }
}

pub trait Bundle {
    fn mount(self, entity: &mut EntityBuilder);
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

#[derive(Debug, Clone)]
pub struct AsyncCommandBuffer {
    cmd: Arc<Mutex<CommandBuffer>>,
}

impl AsyncCommandBuffer {
    pub fn new() -> Self {
        Self {
            cmd: Arc::new(Mutex::new(CommandBuffer::new())),
        }
    }

    pub fn lock(&self) -> MutexGuard<CommandBuffer> {
        self.cmd.lock()
    }
}

impl Default for AsyncCommandBuffer {
    fn default() -> Self {
        Self::new()
    }
}
