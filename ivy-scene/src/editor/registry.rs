use std::{
    any::{Any, TypeId},
    collections::BTreeMap,
    sync::LazyLock,
};

use flax::{component::ComponentDesc, EntityRef};
use futures::StreamExt;
use ivy_ui::{
    streamed::{DuplexComponentStream, Streamed},
    violet::{
        core::{
            state::{State, StateExt, StateSink, StateStream},
            Widget,
        },
        futures_signals::signal::Mutable,
    },
};

use super::editable::Editable;

pub struct EditableRegistry {
    registrations: BTreeMap<TypeId, EditableRegistration>,
}

impl EditableRegistry {
    pub fn new() -> Self {
        let iter = inventory::iter::<EditableRegistration>();

        Self {
            registrations: iter
                .map(|&registration| ((registration.type_id)(), registration))
                .collect(),
        }
    }

    pub fn create_component_editor(
        &self,
        entity: EntityRef,
        component: ComponentDesc,
        streamed: flume::Sender<Box<dyn Streamed>>,
    ) -> Option<Box<dyn Send + Widget>> {
        self.registrations
            .get(&component.type_id())
            .map(|registration| (registration.create_editor)(entity, component, streamed))
    }

    pub fn contains(&self, type_id: TypeId) -> bool {
        self.registrations.contains_key(&type_id)
    }
}

impl Default for EditableRegistry {
    fn default() -> Self {
        Self::new()
    }
}

pub static EDITABLE_REGISTRY: LazyLock<EditableRegistry> = LazyLock::new(EditableRegistry::new);

/// State which yields Box<dyn Value>
pub struct ErasedState<S> {
    state: S,
}

impl<S: State> State for ErasedState<S> {
    type Item = Box<dyn Send + Sync + Any>;
}

impl<S: StateStream> StateStream for ErasedState<S>
where
    S::Item: 'static + Send + Sync,
{
    fn stream(&self) -> futures::stream::BoxStream<'static, Self::Item> {
        Box::pin(
            self.state
                .stream()
                .map(|item| Box::new(item) as Box<dyn Send + Sync + Any>),
        )
    }
}

impl<S: StateSink> StateSink for ErasedState<S>
where
    S::Item: 'static + Send + Sync,
{
    fn send(&self, value: Self::Item) {
        if let Ok(value) = value.downcast::<S::Item>() {
            self.state.send(*value);
        } else {
            panic!("Attempt to send a value of the wrong type");
        }
    }
}

type CreateEditorFunc =
    fn(EntityRef, ComponentDesc, flume::Sender<Box<dyn Streamed>>) -> Box<dyn Send + Widget>;

#[derive(Clone, Copy)]
pub struct EditableRegistration {
    type_id: fn() -> TypeId,
    create_editor: CreateEditorFunc,
}

impl EditableRegistration {
    pub const fn new<T: Clone + Editable + PartialEq>() -> Self {
        Self {
            type_id: || TypeId::of::<T>(),
            create_editor: |entity, component, streamed| {
                let component = component.downcast::<T>();
                let value = entity.get_clone(component).expect("Missing component");
                let state: Mutable<Option<T>> = Mutable::new(Some(value));
                let stream = DuplexComponentStream::new(component, entity.id(), state.clone());

                let _ = streamed.send(Box::new(stream));

                T::create_editor(state.lower_option())
            },
        }
    }
}

#[macro_export]
macro_rules! register_editable {
    ($ty: ty, ) => {
        inventory::submit! {
            $crate::bundle_registry::EditableRegistration::new::<$ty>()
        }
    };
    ($ty: ty, $($rest: tt)*) => {
        register_editable!($ty);
        register_editable!($($rest)*);
    };
    ($ty: ty) => {
        inventory::submit! {
            $crate::editor::registry::EditableRegistration::new::<$ty>()
        }
        inventory::submit! {
            $crate::editor::registry::EditableRegistration::new::<Vec<$ty>>()
        }
    };
}

inventory::collect!(EditableRegistration);
