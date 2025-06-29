use std::{
    any::TypeId,
    collections::BTreeMap,
    sync::{Arc, LazyLock},
};

use async_std::task::sleep;
use bevy_reflect::{PartialReflect, TypeInfo};
use flax::{component::ComponentDesc, EntityRef};
use futures::{stream::BoxStream, StreamExt};
use ivy_ui::{
    streamed::{ComponentSink, Streamed, StreamedUiExt},
    violet::{
        core::{
            state::{StateExt, StateSink, StateStream},
            Scope, Widget,
        },
        futures_signals::signal::Mutable,
    },
};

use crate::editor::editable::{DowncastProject, Projection};

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

    pub fn try_create_editor(
        &self,
        type_id: TypeId,
        state: ProjectedState,
    ) -> Option<Box<dyn Send + Widget>> {
        let registration = EDITABLE_REGISTRY.registrations.get(&type_id)?;

        Some((registration.create_editor)(state))
    }

    pub fn create_component_editor(
        &self,
        entity: EntityRef,
        component: ComponentDesc,
        streamed: flume::Sender<Box<dyn Streamed>>,
    ) -> Option<Box<dyn Send + Widget>> {
        self.registrations
            .get(&component.type_id())
            .map(|registration| (registration.create_component_editor)(entity, component, streamed))
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

type ProjectedState = Box<dyn Projection<Item = dyn PartialReflect>>;
type CreateComponentEditorFunc =
    fn(EntityRef, ComponentDesc, flume::Sender<Box<dyn Streamed>>) -> Box<dyn Send + Widget>;
type CreateEditorFunc = fn(ProjectedState) -> Box<dyn Send + Widget>;

pub trait DowncastableProject {}

#[derive(Clone, Copy)]
pub struct EditableRegistration {
    type_id: fn() -> TypeId,
    create_editor: CreateEditorFunc,
    create_component_editor: CreateComponentEditorFunc,
}

impl EditableRegistration {
    pub const fn new<T: std::fmt::Debug + Clone + Editable + PartialEq>() -> Self {
        Self {
            type_id: || TypeId::of::<T>(),
            create_editor: |project| {
                let concrete = DowncastProject::new(project);

                T::create_editor(concrete)
            },
            create_component_editor: |entity, component, streamed| {
                let component = component.downcast::<T>();
                let value = entity.get_clone(component).expect("Missing component");

                let entity = entity.id();
                let scope = move |scope: &mut Scope| {
                    let state = Mutable::new(value);
                    let new_state = scope.stream_component(component, entity);
                    let feedback_state = Arc::new(state.clone().prevent_feedback());
                    scope.spawn({
                        // Use the feedback preventing state here to avoid sent values from being
                        // sent back to the editor
                        let state = feedback_state.clone();
                        new_state.into_stream().for_each(move |value| {
                            state.send(value);
                            async {}
                        })
                    });

                    let msg = Box::new(ComponentSink::new(
                        component,
                        entity,
                        feedback_state.stream().inspect(move |v| {
                            tracing::info!("Sending value {component} {v:?}");
                        }),
                    )) as Box<dyn Streamed>;

                    let _ = streamed.send(msg);

                    T::create_editor(state).mount(scope);
                };

                Box::new(scope)
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
