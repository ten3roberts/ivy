use std::{
    future::ready,
    marker::Sized,
    sync::{Arc, Mutex},
};

use bevy_reflect::{PartialReflect, TypeInfo};
use flax::{component::ComponentValue, entity_ids, Entity, Query};
use futures::{channel::oneshot, stream::BoxStream, FutureExt, StreamExt};
use glam::{Quat, Vec2, Vec3};
use itertools::Itertools;
use ivy_ui::{
    streamed::StreamedUiExt,
    violet::{
        self,
        core::{
            layout::Align,
            state::{
                Project, State, StateDuplex, StateExt, StateMut, StateSink, StateStream,
                StateStreamRef,
            },
            style::{SizeExt, StyleExt},
            to_owned,
            unit::Unit,
            widget::{
                bold, card, col,
                interactive::{
                    overlay::{overlay_state, Overlay},
                    select_list::SelectList,
                },
                label, row, Button, ButtonStyle, FutureWidget, InputBox, SignalWidget,
                StreamWidget, TextInput,
            },
            Scope, Widget,
        },
        futures_signals::signal::{Mutable, SignalExt},
        lucide::icons::{LUCIDE_CHECK, LUCIDE_PLUS, LUCIDE_TRASH_2, LUCIDE_X},
    },
};

pub use ivy_derive::Editable;

use crate::{editor::registry::EDITABLE_REGISTRY, register_editable};

// pub trait StateDuplex: StateMut + StateStream + StateDuplex {}

// impl<T> StateDuplex for T where T: StateMut + StateStream + StateSink {}

/// A trait for components that can be edited in the editor.
pub trait Editable: 'static + Send + Sync {
    const INLINE: bool;
    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized;
}

impl Editable for String {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        value: S,
    ) -> Box<dyn Send + Widget> {
        Box::new(TextInput::new(value))
    }
}

impl Editable for i32 {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        value: S,
    ) -> Box<dyn Send + Widget> {
        Box::new(InputBox::new(value))
    }
}

impl Editable for f32 {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        value: S,
    ) -> Box<dyn Send + Widget> {
        Box::new(InputBox::new(value))
    }
}

impl Editable for Vec2 {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        value: S,
    ) -> Box<dyn Send + Widget> {
        let value = Arc::new(value.memo(Default::default()));
        let x = value.clone().map_ref(|v| &v.x, |v| &mut v.x);

        let y = value.clone().map_ref(|v| &v.y, |v| &mut v.y);

        Box::new(row((InputBox::new(x), InputBox::new(y))))
    }
}

impl Editable for Vec3 {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        value: S,
    ) -> Box<dyn Send + Widget> {
        let value = Arc::new(value.memo(Default::default()));

        let x = value.clone().map_ref(|v| &v.x, |v| &mut v.x);
        let y = value.clone().map_ref(|v| &v.y, |v| &mut v.y);
        let z = value.clone().map_ref(|v| &v.z, |v| &mut v.z);

        Box::new(row((InputBox::new(x), InputBox::new(y), InputBox::new(z))))
    }
}

impl Editable for Quat {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        value: S,
    ) -> Box<dyn Send + Widget> {
        let value = Arc::new(
            value
                .map_value(
                    |v| v.to_euler(glam::EulerRot::YXZ),
                    |new_value| {
                        Quat::from_euler(glam::EulerRot::YXZ, new_value.0, new_value.1, new_value.2)
                    },
                )
                .memo(Default::default()),
        );

        let yaw = value.clone().map_ref(|v| &v.0, |v| &mut v.0);
        let pitch = value.clone().map_ref(|v| &v.1, |v| &mut v.1);
        let roll = value.clone().map_ref(|v| &v.2, |v| &mut v.2);

        Box::new(row((
            InputBox::new(pitch),
            InputBox::new(yaw),
            InputBox::new(roll),
        )))
    }
}

pub struct EntityDisplay(pub Entity);

impl Widget for EntityDisplay {
    fn mount(self, scope: &mut Scope<'_>) {
        let id = self.0;
        let (name_tx, name_rx) = oneshot::channel();

        scope.apply({
            move |world| {
                if let Ok(entity) = world.entity(self.0) {
                    let _ = name_tx.send(format!("{entity}"));
                } else {
                    let _ = name_tx.send(format!("{id} <dead>"));
                }

                Ok(())
            }
        });

        FutureWidget::new(name_rx.map(|v| v.ok().map(label))).mount(scope);
    }
}

impl Editable for Entity {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        value: S,
    ) -> Box<dyn Send + Widget> {
        let value = Arc::new(value);
        Box::new(
            row((
                StreamWidget::new(value.stream().map(EntityDisplay)),
                Button::label("…").on_click(move |scope| {
                    let value = value.clone();
                    scope
                        .get_context(overlay_state())
                        .open(EntityPicker::new(Box::new(move |id| {
                            value.send(id);
                        })));
                }),
            ))
            .with_cross_align(Align::Center),
        )
    }
}

pub struct EntityPicker {
    on_select: Box<dyn Send + Sync + Fn(Entity)>,
}

impl EntityPicker {
    pub fn new(on_select: Box<dyn Send + Sync + Fn(Entity)>) -> Self {
        Self { on_select }
    }
}

impl Overlay for EntityPicker {
    fn create(
        self,
        scope: &mut Scope<'_>,
        token: violet::core::widget::interactive::overlay::OverlayHandle,
    ) {
        let entities = Mutable::new(Vec::new());

        scope.apply({
            to_owned!(entities);
            move |world| {
                let ids = Query::new(entity_ids()).borrow(world).iter().collect_vec();

                entities.set(ids);

                Ok(())
            }
        });

        let selection: Mutable<Option<usize>> = Mutable::new(None);

        scope.spawn(selection.signal().to_stream().filter_map(ready).for_each({
            to_owned!(entities);
            move |selection| {
                let id = entities.lock_ref()[selection];
                (self.on_select)(id);
                token.close();
                async {}
            }
        }));

        card(SignalWidget::new(entities.signal_ref(move |v| {
            SelectList::new(
                selection.clone(),
                v.iter().copied().map(EntityDisplay).collect_vec(),
            )
        })))
        .with_max_size(Unit::px2(400.0, 400.0))
        .mount(scope);
    }
}

pub struct DowncastProject<U> {
    value: Box<dyn Projection<Item = dyn PartialReflect>>,
    _marker: std::marker::PhantomData<U>,
}

impl<U> DowncastProject<U> {
    pub fn new(value: Box<dyn Projection<Item = dyn PartialReflect>>) -> Self {
        Self {
            value,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<U> State for DowncastProject<U>
where
    U: 'static + Send + Sync,
{
    type Item = U;
}

impl<U> StateStream for DowncastProject<U>
where
    U: 'static + Send + Sync + Clone,
{
    fn stream(&self) -> BoxStream<'static, U> {
        let value = Arc::new(Mutex::new(None));

        Box::pin(
            self.value
                .project_stream({
                    let value = value.clone();
                    Box::new(move |v: &dyn PartialReflect| {
                        *value.lock().unwrap() = Some(v.try_downcast_ref::<U>().unwrap().clone());
                    })
                })
                .map(move |()| value.lock().unwrap().take().unwrap()),
        )
    }
}

impl<U> StateSink for DowncastProject<U>
where
    U: 'static + Send + Sync + Clone,
{
    fn send(&self, new_value: U) {
        let mut new_value = Some(new_value);
        self.value.write(&mut |v| {
            if let Some(v) = v.try_downcast_mut::<U>() {
                *v = new_value.take().unwrap();
            } else {
                panic!("Failed to downcast value to the expected type");
            }
        });
    }
}

/// Represents any type of underlying state projected to a specific type `T`.
pub trait Projection: State + Send + Sync {
    /// Projects this type into another by reference
    fn project<U: ?Sized + 'static>(
        self,
        map_ref: impl 'static + Send + Sync + Fn(&Self::Item) -> &U,
        map_mut: impl 'static + Send + Sync + Fn(&mut Self::Item) -> &mut U,
    ) -> Project<Self, U>
    where
        Self: Sized;

    /// Stream projected values
    fn project_stream(
        &self,
        map_ref: Box<dyn 'static + Send + Sync + Fn(&Self::Item)>,
    ) -> BoxStream<'static, ()>;

    /// Writ a new value directly to the projection of the underlying state.
    fn write(&self, writer: &mut dyn FnMut(&mut Self::Item));
}

impl<T: Send + Sync> Projection for Mutable<T>
where
    T: 'static + Send + Sync,
{
    fn project<U: ?Sized + 'static>(
        self,
        map_ref: impl 'static + Send + Sync + Fn(&T) -> &U,
        map_mut: impl 'static + Send + Sync + Fn(&mut T) -> &mut U,
    ) -> Project<Self, U>
    where
        Self: Sized,
    {
        Project::new_dyn(self, map_ref, map_mut)
    }

    fn project_stream(
        &self,
        func: Box<dyn 'static + Send + Sync + Fn(&Self::Item)>,
    ) -> BoxStream<'static, ()> {
        self.stream_ref(func).boxed()
    }

    fn write(&self, writer: &mut dyn FnMut(&mut Self::Item)) {
        self.write_mut(writer);
    }
}

impl<C, T, F, G> Projection for Project<C, T, F, G>
where
    C: Send + Sync + StateMut + StateStreamRef,
    T: ?Sized + 'static + Send + Sync,
    F: ?Sized + 'static + Send + Sync + Fn(&C::Item) -> &T,
    G: ?Sized + 'static + Send + Sync + Fn(&mut C::Item) -> &mut T,
{
    fn project<V: ?Sized + 'static>(
        self,
        map_ref: impl 'static + Send + Sync + Fn(&T) -> &V,
        map_mut: impl 'static + Send + Sync + Fn(&mut T) -> &mut V,
    ) -> Project<Self, V>
    where
        Self: Sized,
    {
        Project::new_dyn(self, map_ref, map_mut)
    }

    fn project_stream(
        &self,
        func: Box<dyn 'static + Send + Sync + Fn(&Self::Item)>,
    ) -> BoxStream<'static, ()>
    where
        Self: Sized,
    {
        self.stream_ref(func).boxed()
    }

    fn write(&self, writer: &mut dyn FnMut(&mut Self::Item))
    where
        Self: Sized,
    {
        self.write_mut(writer)
    }
}

impl Projection for Box<dyn Projection<Item = dyn PartialReflect>> {
    fn project<V: ?Sized + 'static>(
        self,
        map_ref: impl 'static + Send + Sync + Fn(&Self::Item) -> &V,
        map_mut: impl 'static + Send + Sync + Fn(&mut Self::Item) -> &mut V,
    ) -> Project<Self, V>
    where
        Self: Sized,
    {
        Project::new_dyn(self, map_ref, map_mut)
    }

    fn project_stream(
        &self,
        func: Box<dyn 'static + Send + Sync + Fn(&Self::Item)>,
    ) -> BoxStream<'static, ()> {
        (**self).project_stream(func)
    }

    fn write(&self, writer: &mut dyn FnMut(&mut Self::Item)) {
        (**self).write(writer);
    }
}

impl<T: Projection> Projection for Arc<T> {
    fn project<V: ?Sized + 'static>(
        self,
        map_ref: impl 'static + Send + Sync + Fn(&Self::Item) -> &V,
        map_mut: impl 'static + Send + Sync + Fn(&mut Self::Item) -> &mut V,
    ) -> Project<Self, V>
    where
        Self: Sized,
    {
        Project::new_dyn(self, map_ref, map_mut)
    }

    fn project_stream(
        &self,
        func: Box<dyn 'static + Send + Sync + Fn(&Self::Item)>,
    ) -> BoxStream<'static, ()> {
        (**self).project_stream(func)
    }

    fn write(&self, writer: &mut dyn FnMut(&mut Self::Item)) {
        (**self).write(writer);
    }
}

/// Creates an editor from a type using type reflection.
pub fn create_reflected_editor<T>(type_info: &TypeInfo, state: T) -> Box<dyn Widget>
where
    T: 'static + Send + Sync + StateMut + StateStreamRef,
    <T as State>::Item: PartialReflect,
    <T as State>::Item: 'static + Send + Sync + Sized,
{
    let state = Project::<Arc<T>, dyn PartialReflect>::new_dyn(
        Arc::new(state),
        |v| v as &dyn PartialReflect,
        |v| v as &mut dyn PartialReflect,
    );

    reflection_editor(type_info, state)
}

fn reflection_editor<T>(
    type_info: &TypeInfo,
    value: Project<T, dyn PartialReflect>,
) -> Box<dyn Widget>
where
    T: StateMut + StateStreamRef + Clone,
    T: 'static + Send + Sync,
{
    // Always prefer concrete editors
    if let Some(editor) =
        EDITABLE_REGISTRY.try_create_editor(type_info.type_id(), Box::new(value.clone()))
    {
        return editor;
    }
    match type_info {
        TypeInfo::Struct(struct_info) => {
            let fields = struct_info.field_names().iter().map(|field_name| {
                let ty = struct_info.field(&field_name).unwrap();
                let field_value = value.clone().flat_project(
                    |v| {
                        v.reflect_ref()
                            .as_struct()
                            .unwrap()
                            .field(field_name)
                            .unwrap()
                    },
                    |v| {
                        v.reflect_mut()
                            .as_struct()
                            .unwrap()
                            .field_mut(field_name)
                            .unwrap()
                    },
                );

                row((
                    bold(format!("{field_name}:")),
                    reflection_editor(ty.type_info().unwrap(), field_value),
                ))
            });

            Box::new(col(fields.collect_vec()))
        }
        TypeInfo::TupleStruct(_tuple_struct_info) => todo!(),
        TypeInfo::Tuple(_tuple_info) => todo!(),
        TypeInfo::List(_list_info) => todo!(),
        TypeInfo::Array(_array_info) => todo!(),
        TypeInfo::Map(_map_info) => todo!(),
        TypeInfo::Set(_set_info) => todo!(),
        TypeInfo::Enum(_enum_info) => todo!(),
        TypeInfo::Opaque(_opaque_info) => Box::new(label(_opaque_info.type_path())),
    }
}

impl<T> Editable for Vec<T>
where
    T: ComponentValue + Editable + Clone,
{
    const INLINE: bool = false;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        value: S,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        let value = Arc::new(value.memo(Default::default()));
        let values = value.stream().map({
            to_owned!(value);
            move |values| {
                let value = value.clone();
                let values = values
                    .iter()
                    .enumerate()
                    .map(|(i, _)| {
                        let element = value
                            .clone()
                            .memo(Default::default())
                            .transform(move |v| v[i].clone(), move |v, new_value| v[i] = new_value);

                        let value = value.clone();
                        row((
                            T::create_editor(element),
                            Button::label(LUCIDE_TRASH_2)
                                .with_style(ButtonStyle::hidden())
                                .with_tooltip_text("Remove item")
                                .on_click(move |_| {
                                    value.write_mut(|v| v.remove(i));
                                }),
                        ))
                        .with_cross_align(Align::Center)
                    })
                    .collect_vec();

                col(values)
            }
        });

        let (add_widget_tx, add_widget_rx) = flume::unbounded::<Option<Box<dyn Send + Widget>>>();
        let _ = add_widget_tx.send(None);

        Box::new(col((
            StreamWidget::new(values),
            StreamWidget::new(add_widget_rx.into_stream().map(move |v| {
                to_owned!(value, add_widget_tx);
                v.unwrap_or_else(move || {
                    to_owned!(value);
                    let new_button = Button::label(LUCIDE_PLUS)
                        .with_style(ButtonStyle::hidden())
                        .with_tooltip_text("Add new item")
                        .on_click({
                            to_owned!(value);
                            move |_| {
                                to_owned!(value);
                                let new_value = Mutable::new(None as Option<T>);

                                let editor = T::create_editor(new_value.clone().lower_option());
                                let confirm = Button::label(LUCIDE_CHECK)
                                    .with_style(ButtonStyle::hidden())
                                    .on_click({
                                        to_owned!(add_widget_tx);
                                        move |_| {
                                            if let Some(new_value) = new_value.lock_ref().clone() {
                                                let _ = add_widget_tx.send(None);
                                                value.write_mut(|v| v.push(new_value));
                                            }
                                        }
                                    });
                                let abort = Button::label(LUCIDE_X)
                                    .with_style(ButtonStyle::hidden())
                                    .with_tooltip_text("Cancel")
                                    .on_click({
                                        to_owned!(add_widget_tx);
                                        move |_| {
                                            let _ = add_widget_tx.send(None);
                                        }
                                    });

                                let editor = row((editor, abort, confirm));

                                let _ = add_widget_tx.send(Some(Box::new(editor)));
                            }
                        });

                    Box::new(new_button)
                })
            })),
        )))
    }
}

register_editable!(String, f32, i32, Entity, Vec2, Vec3, Quat);

#[doc(hidden)]
pub mod __private {
    pub use ivy_ui::violet;
}
