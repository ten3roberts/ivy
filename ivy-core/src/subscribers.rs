use flax::{
    component::{ComponentDesc, ComponentKey, ComponentValue},
    events::EventSubscriber,
    sink::Sink,
    Component, Entity, RelationExt,
};

pub struct RemovedComponentSubscriber<T> {
    tx: flume::Sender<(Entity, T)>,
    component: ComponentKey,
}

impl<T> RemovedComponentSubscriber<T> {
    pub fn new(tx: flume::Sender<(Entity, T)>, component: Component<T>) -> Self
    where
        T: ComponentValue,
    {
        Self {
            tx,
            component: component.key(),
        }
    }
}

impl<T: ComponentValue + Clone> EventSubscriber for RemovedComponentSubscriber<T> {
    fn on_added(&self, _: &flax::archetype::ArchetypeStorage, _: &flax::events::EventData) {}

    fn on_modified(&self, _: &flax::events::EventData) {}

    fn on_removed(
        &self,
        storage: &flax::archetype::ArchetypeStorage,
        event: &flax::events::EventData,
    ) {
        let storage = storage.downcast_ref::<T>();
        for (&id, slot) in event.ids.iter().zip(event.slots) {
            self.tx.send((id, storage[slot].clone())).ok();
        }
    }

    fn matches_arch(&self, arch: &flax::archetype::Archetype) -> bool {
        arch.has(self.component)
    }

    fn matches_component(&self, v: ComponentDesc) -> bool {
        v.key() == self.component
    }

    fn is_connected(&self) -> bool {
        self.tx.is_connected()
    }
}

pub struct RemovedRelationSubscriber<T> {
    tx: flume::Sender<(Entity, Component<T>, T)>,
    id: Entity,
}

impl<T> RemovedRelationSubscriber<T> {
    pub fn new(tx: flume::Sender<(Entity, Component<T>, T)>, relation: impl RelationExt<T>) -> Self
    where
        T: ComponentValue,
    {
        Self {
            tx,
            id: relation.id(),
        }
    }
}

impl<T: ComponentValue + Copy> EventSubscriber for RemovedRelationSubscriber<T> {
    fn on_added(&self, _: &flax::archetype::ArchetypeStorage, _: &flax::events::EventData) {}

    fn on_modified(&self, _: &flax::events::EventData) {}

    fn on_removed(
        &self,
        storage: &flax::archetype::ArchetypeStorage,
        event: &flax::events::EventData,
    ) {
        let values = storage.downcast_ref::<T>();
        for (&id, slot) in event.ids.iter().zip(event.slots) {
            self.tx
                .send((id, storage.desc().downcast(), values[slot]))
                .ok();
        }
    }

    fn matches_arch(&self, arch: &flax::archetype::Archetype) -> bool {
        arch.relations_like(self.id).any(|_| true)
    }

    fn matches_component(&self, v: ComponentDesc) -> bool {
        v.key().id() == self.id
    }

    fn is_connected(&self) -> bool {
        self.tx.is_connected()
    }
}
