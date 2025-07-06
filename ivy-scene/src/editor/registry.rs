use std::{
    any::{Any, TypeId},
    collections::BTreeMap,
    sync::{Arc, LazyLock},
    time::Duration,
};

use bevy_reflect::{PartialReflect, TypeInfo};
use flax::{component::ComponentDesc, EntityRef};
use futures::{stream::BoxStream, StreamExt};
use ivy_ui::{
    streamed::{ComponentSink, Streamed, StreamedUiExt},
    violet::{
        core::{
            state::{StateExt, StateSink, StateStream},
            time::sleep,
            utils::throttle_skip,
            Scope, Widget,
        },
        futures_signals::signal::Mutable,
    },
};

use crate::editor::editable::{DowncastProject, Projection};

use super::editable::Editable;

pub struct EditableRegistry {
    registrations: BTreeMap<TypeId, EditableRegistration>,
    named: BTreeMap<&'static str, EditableRegistration>,
}

impl EditableRegistry {
    pub fn new() -> Self {
        let iter = inventory::iter::<EditableRegistration>();

        let registrations: BTreeMap<_, _> = iter
            .map(|&registration| ((registration.type_id)(), registration))
            .collect();

        Self {
            named: registrations
                .iter()
                .filter_map(|(_, registration)| {
                    Some((registration.type_name?, registration.clone()))
                })
                .collect(),
            registrations,
        }
    }

    pub fn get_by_name(&self, name: &str) -> Option<&EditableRegistration> {
        self.named.get(name)
    }

    pub fn try_create_editor(
        &self,
        type_id: TypeId,
        state: ProjectedState,
    ) -> Option<Box<dyn Send + Widget>> {
        let registration = EDITABLE_REGISTRY.registrations.get(&type_id)?;

        Some((registration.create_editor_reflected)(state))
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
type CreateEditorAny = fn(Mutable<Box<dyn Send + Sync + Any>>) -> Box<dyn Send + Widget>;

pub trait DowncastableProject {}

#[derive(Clone, Copy)]
pub struct EditableRegistration {
    type_name: Option<&'static str>,
    type_id: fn() -> TypeId,
    create_editor_reflected: CreateEditorFunc,
    create_editor_boxed: CreateEditorAny,
    create_component_editor: CreateComponentEditorFunc,
}

impl EditableRegistration {
    pub const fn new<T: std::fmt::Debug + Clone + Editable + PartialEq>(
        name: Option<&'static str>,
    ) -> Self {
        Self {
            type_name: name,
            type_id: || TypeId::of::<T>(),
            create_editor_reflected: |project| {
                let concrete = DowncastProject::new(project);

                T::create_editor(concrete)
            },
            create_editor_boxed: |value| {
                todo!()
                // let concrete = DowncastProject::new(project);

                // T::create_editor(concrete)
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
                        throttle_skip(new_state.into_stream(), || {
                            sleep(Duration::from_millis(1000))
                        })
                        .for_each(move |value| {
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

    pub fn create_editor(
        &self,
        value: Mutable<Box<dyn Send + Sync + Any>>,
    ) -> Box<dyn Send + Widget> {
        (self.create_editor_boxed)(value)
    }
}

#[macro_export]
macro_rules! register_editable {
    ($ty: ty, $($rest: tt)*) => {
        register_editable!($ty);
        register_editable!($($rest)*);
    };
    ($ty: ty) => {
        inventory::submit! {
            $crate::editor::registry::EditableRegistration::new::<$ty>(Some(stringify!($ty)))
        }
        inventory::submit! {
            $crate::editor::registry::EditableRegistration::new::<Vec<$ty>>(None)
        }
    };
}

inventory::collect!(EditableRegistration);
