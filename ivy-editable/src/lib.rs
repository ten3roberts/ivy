use std::{
    future::ready,
    marker::Sized,
    sync::{Arc, Mutex},
};

use bevy_reflect::{PartialReflect, TypeInfo};
use flax::{Entity, component::ComponentValue};
use futures::{StreamExt, stream::BoxStream};
use glam::{Quat, Vec2, Vec3};
use itertools::Itertools;
use violet::{
    self,
    core::{
        Scope, Widget,
        layout::Align,
        state::{
            Project, State, StateDuplex, StateExt, StateSink, StateStream, StateStreamRef,
            StateWrite,
        },
        style::{SizeExt, StyleExt},
        to_owned,
        unit::Unit,
        widget::{
            Button, ButtonStyle, Checkbox, InputBox, SignalWidget, StreamWidget, TextInput, bold,
            card, col,
            interactive::{
                colorpicker::RgbColorPicker,
                overlay::{Overlay, overlay_state},
                select_list::SelectList,
            },
            label, row,
        },
    },
    futures_signals::signal::{Mutable, SignalExt},
    lucide::icons::{LUCIDE_CHECK, LUCIDE_PLUS, LUCIDE_TRASH_2, LUCIDE_X},
    palette::{Srgb, Srgba, WithAlpha},
};

pub mod registry;

use crate::registry::EDITABLE_REGISTRY;

/// A trait for components that can be edited in the editor.
pub trait Editable: 'static + Send + Sync {
    const INLINE: bool;
    /// Create an editor appropriate for this type.
    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized;

    /// Reference projection variant of [`create_editor`] which allows direct access to state and
    /// further subprojection without cloning.
    fn create_editor_project<
        S: 'static + Send + Sync + Clone + StateStreamRef<Item = Self> + StateWrite,
    >(
        state: S,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized;
}

pub use ivy_derive::Editable;

impl Editable for String {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
    ) -> Box<dyn Send + Widget> {
        Box::new(TextInput::new(state.memo(Default::default()).dedup()))
    }

    fn create_editor_project<S: 'static + Send + Sync + StateStreamRef<Item = Self> + StateWrite>(
        state: S,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Self::create_editor(state.project_ref(|v| v, |v| v))
    }
}

impl Editable for i32 {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
    ) -> Box<dyn Send + Widget> {
        Box::new(InputBox::new(state.memo(Default::default()).dedup()))
    }

    fn create_editor_project<S: 'static + Send + Sync + StateStreamRef<Item = Self> + StateWrite>(
        state: S,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Box::new(InputBox::new(state.project_ref(|v| v, |v| v)))
    }
}

impl Editable for bool {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
    ) -> Box<dyn Send + Widget> {
        Box::new(Checkbox::new(state.memo(false).dedup()))
    }

    fn create_editor_project<S: 'static + Send + Sync + StateStreamRef<Item = Self> + StateWrite>(
        state: S,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Box::new(Checkbox::new(state.project_ref(|v| v, |v| v)))
    }
}

impl Editable for f32 {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
    ) -> Box<dyn Send + Widget> {
        Box::new(InputBox::new(state.memo(Default::default()).dedup()))
    }

    fn create_editor_project<S: 'static + Send + Sync + StateStreamRef<Item = Self> + StateWrite>(
        state: S,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Box::new(InputBox::new(state.project_ref(|v| v, |v| v)))
    }
}

impl Editable for Vec2 {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
    ) -> Box<dyn Send + Widget> {
        Self::create_editor_project(Arc::new(state.memo(Default::default())))
    }

    fn create_editor_project<
        S: 'static + Send + Sync + Clone + StateStreamRef<Item = Self> + StateWrite,
    >(
        state: S,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        let x = state.clone().project_ref(|v| &v.x, |v| &mut v.x);
        let y = state.clone().project_ref(|v| &v.y, |v| &mut v.y);

        Box::new(row((InputBox::new(x), InputBox::new(y))))
    }
}

impl Editable for Vec3 {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
    ) -> Box<dyn Send + Widget> {
        Self::create_editor_project(Arc::new(state.memo(Default::default())))
    }

    fn create_editor_project<
        S: 'static + Send + Sync + Clone + StateStreamRef<Item = Self> + StateWrite,
    >(
        state: S,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        let x = state.clone().project_ref(|v| &v.x, |v| &mut v.x);
        let y = state.clone().project_ref(|v| &v.y, |v| &mut v.y);
        let z = state.clone().project_ref(|v| &v.z, |v| &mut v.z);

        Box::new(row((InputBox::new(x), InputBox::new(y), InputBox::new(z))))
    }
}

impl Editable for Srgb {
    const INLINE: bool = false;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
    ) -> Box<dyn Send + Widget> {
        Box::new(
            RgbColorPicker::new(state.map_value(|v| v.with_alpha(1.0), |v| v.without_alpha()))
                .enable_alpha(false),
        )
    }

    fn create_editor_project<S: 'static + Send + Sync + StateStreamRef<Item = Self> + StateWrite>(
        state: S,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Self::create_editor(state.project_ref(|v| v, |v| v))
    }
}

impl Editable for Srgba {
    const INLINE: bool = false;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
    ) -> Box<dyn Send + Widget> {
        Box::new(RgbColorPicker::new(state).enable_alpha(true))
    }

    fn create_editor_project<S: 'static + Send + Sync + StateStreamRef<Item = Self> + StateWrite>(
        state: S,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Self::create_editor(state.project_ref(|v| v, |v| v))
    }
}

impl Editable for Quat {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
    ) -> Box<dyn Send + Widget> {
        let state = Arc::new(
            state
                .map_value(
                    |v| {
                        let (a, b, c) = v.to_euler(glam::EulerRot::YXZ);
                        (to_degrees(a), to_degrees(b), to_degrees(c))
                    },
                    |new_value| {
                        Quat::from_euler(
                            glam::EulerRot::YXZ,
                            to_radians(new_value.0),
                            to_radians(new_value.1),
                            to_radians(new_value.2),
                        )
                    },
                )
                .memo(Default::default()),
        );

        fn to_degrees(radians: f32) -> f32 {
            radians.to_degrees()
        }

        fn to_radians(degrees: f32) -> f32 {
            degrees.to_radians()
        }

        let yaw = state.clone().project_ref(|v| &v.0, |v| &mut v.0);
        let pitch = state.clone().project_ref(|v| &v.1, |v| &mut v.1);
        let roll = state.clone().project_ref(|v| &v.2, |v| &mut v.2);

        Box::new(row((
            InputBox::new(pitch),
            InputBox::new(yaw),
            InputBox::new(roll),
        )))
    }

    fn create_editor_project<S: 'static + Send + Sync + StateStreamRef<Item = Self> + StateWrite>(
        state: S,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Self::create_editor(state.project_ref(|v| v, |v| v))
    }
}

pub struct EntityDisplay(pub Entity);

impl Widget for EntityDisplay {
    fn mount(self, _: &mut Scope<'_>) {
        todo!()
        // let (name_tx, name_rx) = oneshot::channel();

        // scope.apply({
        //     move |world| {
        //         if let Ok(entity) = world.entity(self.0) {
        //             let _ = name_tx.send(format!("{entity}"));
        //         } else {
        //             let _ = name_tx.send(format!("{id} <dead>"));
        //         }

        //         Ok(())
        //     }
        // });

        // FutureWidget::new(name_rx.map(|v| v.ok().map(label))).mount(scope);
    }
}

impl Editable for Entity {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
    ) -> Box<dyn Send + Widget> {
        let state = Arc::new(state);
        Box::new(
            row((
                StreamWidget::new(state.stream().map(EntityDisplay)),
                Button::label("…").on_click(move |scope| {
                    let state = state.clone();
                    scope
                        .get_context(overlay_state())
                        .open(EntityPicker::new(Box::new(move |id| {
                            state.send(id);
                        })));
                }),
            ))
            .with_cross_align(Align::Center),
        )
    }

    fn create_editor_project<S: 'static + Send + Sync + StateStreamRef<Item = Self> + StateWrite>(
        state: S,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Self::create_editor(state.project_ref(|v| v, |v| v))
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

        // scope.apply({
        //     to_owned!(entities);
        //     move |world| {
        //         let ids = Query::new(entity_ids()).borrow(world).iter().collect_vec();

        //         entities.set(ids);

        //         Ok(())
        //     }
        // });

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

pub struct DowncastPartialReflect<U> {
    state: Box<dyn Projection<Item = dyn PartialReflect>>,
    _marker: std::marker::PhantomData<U>,
}

impl<U> DowncastPartialReflect<U> {
    pub fn new(state: Box<dyn Projection<Item = dyn PartialReflect>>) -> Self {
        Self {
            state,
            _marker: std::marker::PhantomData,
        }
    }
}

impl<U> State for DowncastPartialReflect<U>
where
    U: 'static + Send + Sync,
{
    type Item = U;
}

impl<U> StateStream for DowncastPartialReflect<U>
where
    U: 'static + Send + Sync + Clone,
{
    fn stream(&self) -> BoxStream<'static, U> {
        let state = Arc::new(Mutex::new(None));

        Box::pin(
            self.state
                .project_stream({
                    let state = state.clone();
                    Box::new(move |v: &dyn PartialReflect| {
                        *state.lock().unwrap() = Some(v.try_downcast_ref::<U>().unwrap().clone());
                    })
                })
                .map(move |()| state.lock().unwrap().take().unwrap()),
        )
    }
}

impl<U> StateSink for DowncastPartialReflect<U>
where
    U: 'static + Send + Sync + Clone,
{
    fn send(&self, new_value: U) {
        let mut new_value = Some(new_value);
        self.state.write(&mut |v| {
            if let Some(v) = v.try_downcast_mut::<U>() {
                *v = new_value.take().unwrap();
            } else {
                panic!("Failed to downcast state to the expected type");
            }
        });
    }
}

/// Represents any type of underlying state projected to a specific type `T`.
pub trait Projection: State + Send + Sync {
    /// Projects this type into another by reference
    fn project<U: ?Sized + 'static>(
        self,
        project_ref: impl 'static + Send + Sync + Fn(&Self::Item) -> &U,
        map_mut: impl 'static + Send + Sync + Fn(&mut Self::Item) -> &mut U,
    ) -> Project<Self, U>
    where
        Self: Sized;

    /// Stream projected values
    fn project_stream(
        &self,
        project_ref: Box<dyn 'static + Send + Sync + Fn(&Self::Item)>,
    ) -> BoxStream<'static, ()>;

    /// Writ a new state directly to the projection of the underlying state.
    fn write(&self, writer: &mut dyn FnMut(&mut Self::Item));
}

impl<T: Send + Sync> Projection for Mutable<T>
where
    T: 'static + Send + Sync,
{
    fn project<U: ?Sized + 'static>(
        self,
        project_ref: impl 'static + Send + Sync + Fn(&T) -> &U,
        map_mut: impl 'static + Send + Sync + Fn(&mut T) -> &mut U,
    ) -> Project<Self, U>
    where
        Self: Sized,
    {
        Project::new_dyn(self, project_ref, map_mut)
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
    C: Send + Sync + StateWrite + StateStreamRef,
    T: ?Sized + 'static + Send + Sync,
    F: 'static + Send + Sync + Fn(&C::Item) -> &T,
    G: 'static + Send + Sync + Fn(&mut C::Item) -> &mut T,
{
    fn project<V: ?Sized + 'static>(
        self,
        project_ref: impl 'static + Send + Sync + Fn(&T) -> &V,
        map_mut: impl 'static + Send + Sync + Fn(&mut T) -> &mut V,
    ) -> Project<Self, V>
    where
        Self: Sized,
    {
        Project::new_dyn(self, project_ref, map_mut)
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
        project_ref: impl 'static + Send + Sync + Fn(&Self::Item) -> &V,
        map_mut: impl 'static + Send + Sync + Fn(&mut Self::Item) -> &mut V,
    ) -> Project<Self, V>
    where
        Self: Sized,
    {
        Project::new_dyn(self, project_ref, map_mut)
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
        project_ref: impl 'static + Send + Sync + Fn(&Self::Item) -> &V,
        map_mut: impl 'static + Send + Sync + Fn(&mut Self::Item) -> &mut V,
    ) -> Project<Self, V>
    where
        Self: Sized,
    {
        Project::new_dyn(self, project_ref, map_mut)
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
    T: 'static + Send + Sync + StateWrite + StateStreamRef,
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
    state: Project<T, dyn PartialReflect>,
) -> Box<dyn Widget>
where
    T: StateWrite + StateStreamRef + Clone,
    T: 'static + Send + Sync,
{
    // Always prefer concrete editors
    if let Some(editor) =
        EDITABLE_REGISTRY.try_create_editor(type_info.type_id(), Box::new(state.clone()))
    {
        return editor;
    }
    match type_info {
        TypeInfo::Struct(struct_info) => {
            let fields = struct_info.field_names().iter().map(|field_name| {
                let ty = struct_info.field(&field_name).unwrap();
                let field_value = state.clone().flat_project(
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
        state: S,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        let state = Arc::new(state.memo(Default::default()));
        let values = state.stream().map({
            to_owned!(state);
            move |values| {
                let state = state.clone();
                let values = values
                    .iter()
                    .enumerate()
                    .map(|(i, _)| {
                        let element = state
                            .clone()
                            .transform(move |v| v[i].clone(), move |v, new_value| v[i] = new_value);

                        let state = state.clone();
                        row((
                            T::create_editor(element),
                            Button::label(LUCIDE_TRASH_2)
                                .with_style(ButtonStyle::hidden())
                                .with_tooltip_text("Remove item")
                                .on_click(move |_| {
                                    state.write_mut(|v| v.remove(i));
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
                to_owned!(state, add_widget_tx);
                v.unwrap_or_else(move || {
                    to_owned!(state);
                    let new_button = Button::label(LUCIDE_PLUS)
                        .with_style(ButtonStyle::hidden())
                        .with_tooltip_text("Add new item")
                        .on_click({
                            to_owned!(state);
                            move |_| {
                                to_owned!(state);
                                let new_value = Mutable::new(None as Option<T>);

                                let editor = T::create_editor(new_value.clone().lower_option());
                                let confirm = Button::label(LUCIDE_CHECK)
                                    .with_style(ButtonStyle::hidden())
                                    .on_click({
                                        to_owned!(add_widget_tx);
                                        move |_| {
                                            if let Some(new_value) = new_value.lock_ref().clone() {
                                                let _ = add_widget_tx.send(None);
                                                state.write_mut(|v| v.push(new_value));
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

    fn create_editor_project<S: 'static + Send + Sync + StateStreamRef<Item = Self> + StateWrite>(
        state: S,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Self::create_editor(state.project_ref(|v| v, |v| v))
    }
}

register_editable!(String, f32, i32, Entity, Vec2, Vec3, Quat);

#[doc(hidden)]
pub mod __private {
    pub use inventory;
    pub use violet;
}
