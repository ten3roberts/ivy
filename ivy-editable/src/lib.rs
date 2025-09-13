use std::{collections::BTreeMap, fmt::Display, marker::Sized, sync::Arc};

use flax::{Entity, component::ComponentValue};
use futures::{StreamExt, stream::BoxStream};
use glam::{Quat, Vec2, Vec3};
use itertools::Itertools;
use violet::{
    core::{
        Widget,
        layout::Align,
        state::{
            Project, State, StateDuplex, StateExt, StateProjected, StateStream, StateStreamRef,
            StateWrite,
        },
        style::{StyleExt, element_disabled, element_primary},
        to_owned,
        widget::{Button, ButtonStyle, SignalWidget, StreamWidget, col, label, row},
    },
    futures_signals::signal::{Mutable, SignalExt},
    lucide::icons::{LUCIDE_CHECK, LUCIDE_PLUS, LUCIDE_TRASH_2, LUCIDE_X},
};

mod impls;
pub mod registry;

#[doc(hidden)]
pub use ivy_assets::AssetCache;

#[derive(Clone, Copy, Debug)]
pub struct EditorOpts<T> {
    pub range: Option<(T, T)>,
}

impl<T> EditorOpts<T> {
    fn map_range<F, U>(self, f: F) -> EditorOpts<U>
    where
        F: Fn(T) -> U,
        T: Clone,
        U: Clone,
    {
        EditorOpts {
            range: self.range.map(|(min, max)| (f(min), f(max))),
        }
    }
}

impl<T> Default for EditorOpts<T> {
    fn default() -> Self {
        Self { range: None }
    }
}

/// A trait for components that can be edited in the editor.
pub trait Editable: 'static + Send + Sync {
    const INLINE: bool;
    /// Create an editor appropriate for this type.
    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized;

    /// Reference projection variant of [`create_editor`] which allows direct access to state and
    /// further subprojection without cloning.
    fn create_editor_project<S: 'static + Send + Sync + Clone + StateProjected<Item = Self>>(
        state: S,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized;
}

/// A trait for components that can be edited in the editor.
pub trait EditableWithOpts: 'static + Send + Sync + Editable {
    type Range;

    const INLINE: bool;
    /// Create an editor appropriate for this type.
    fn create_editor_opts<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
        opts: EditorOpts<Self::Range>,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized;

    /// Reference projection variant of [`create_editor`] which allows direct access to state and
    /// further subprojection without cloning.
    fn create_editor_project_opts<S: 'static + Send + Sync + Clone + StateProjected<Item = Self>>(
        state: S,
        opts: EditorOpts<Self::Range>,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized;
}

impl<T: EditableWithOpts> Editable for T {
    const INLINE: bool = <T as EditableWithOpts>::INLINE;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Self::create_editor_opts(state, EditorOpts::default(), assets)
    }

    fn create_editor_project<S: 'static + Send + Sync + Clone + StateProjected<Item = Self>>(
        state: S,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Self::create_editor_project_opts(state, EditorOpts::default(), assets)
    }
}

pub use ivy_derive::Editable;

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

// impl Projection for Box<dyn Projection<Item = dyn PartialReflect>> {
//     fn project<V: ?Sized + 'static>(
//         self,
//         project_ref: impl 'static + Send + Sync + Fn(&Self::Item) -> &V,
//         map_mut: impl 'static + Send + Sync + Fn(&mut Self::Item) -> &mut V,
//     ) -> Project<Self, V>
//     where
//         Self: Sized,
//     {
//         Project::new_dyn(self, project_ref, map_mut)
//     }

//     fn project_stream(
//         &self,
//         func: Box<dyn 'static + Send + Sync + Fn(&Self::Item)>,
//     ) -> BoxStream<'static, ()> {
//         (**self).project_stream(func)
//     }

//     fn write(&self, writer: &mut dyn FnMut(&mut Self::Item)) {
//         (**self).write(writer);
//     }
// }

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

impl<T> Editable for Vec<T>
where
    T: ComponentValue + Editable + Clone,
{
    const INLINE: bool = false;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        let state = Arc::new(state.memo(Default::default()));
        let values = state.stream().map({
            to_owned!(state, assets);
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
                            T::create_editor(element, &assets),
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

        to_owned!(assets);
        Box::new(col((
            StreamWidget::new(values),
            StreamWidget::new(add_widget_rx.into_stream().map(move |v| {
                to_owned!(assets, state, add_widget_tx);
                v.unwrap_or_else(move || {
                    to_owned!(assets, state);
                    let new_button = Button::label(LUCIDE_PLUS)
                        .with_style(ButtonStyle::hidden())
                        .with_tooltip_text("Add new item")
                        .on_click({
                            to_owned!(state, assets);
                            move |_| {
                                to_owned!(state, assets);
                                let new_value = Mutable::new(None as Option<T>);

                                let editor =
                                    T::create_editor(new_value.clone().lower_option(), &assets);
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
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Self::create_editor(state.project_ref(|v| v, |v| v), assets)
    }
}

impl<K: Ord + Eq + Editable + Clone + Display, V: Editable + Clone> Editable for BTreeMap<K, V> {
    const INLINE: bool = false;

    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        let state = Arc::new(state.memo(Default::default()));
        state.sync_initial();
        to_owned!(assets);

        let values = state.stream().map({
            to_owned!(assets, state);
            move |values| {
                let state = state.clone();
                let values = values
                    .iter()
                    .map(|(k, _)| {
                        let k_display = k.to_string();
                        let k = k.to_owned();
                        let element = state.clone().transform(
                            {
                                to_owned!(k);
                                move |v| v[&k].clone()
                            },
                            {
                                to_owned!(k);
                                move |v, new_value| *v.get_mut(&k).unwrap() = new_value
                            },
                        );

                        let state = state.clone();
                        row((
                            label(k_display),
                            V::create_editor(element, &assets),
                            Button::label(LUCIDE_TRASH_2)
                                .with_style(ButtonStyle::hidden())
                                .with_tooltip_text("Remove item")
                                .on_click(move |_| {
                                    state.write_mut(|v| v.remove(&k));
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

        let new_entry = add_widget_rx.into_stream().map(move |v| {
            to_owned!(assets, state, add_widget_tx);
            v.unwrap_or_else(move || {
                to_owned!(state, assets);
                let new_button = Button::label(LUCIDE_PLUS)
                    .with_style(ButtonStyle::hidden())
                    .with_tooltip_text("Add new item")
                    .on_click({
                        to_owned!(state);
                        move |_| {
                            to_owned!(state);
                            let new_entry = Mutable::new((None, None) as (Option<K>, Option<V>));
                            let new_key = new_entry
                                .clone()
                                .project_ref(|v| &v.0, |v| &mut v.0)
                                .lower_option();

                            let new_value = new_entry
                                .clone()
                                .project_ref(|v| &v.1, |v| &mut v.1)
                                .lower_option();

                            let key_editor = K::create_editor(new_key, &assets);
                            let value_editor = V::create_editor(new_value, &assets);
                            to_owned!(add_widget_tx, state);
                            let confirm = new_entry
                                .signal_ref(|(k, v)| (k.is_some() && v.is_some()))
                                .map({
                                    to_owned!(add_widget_tx);
                                    move |valid| {
                                        to_owned!(new_entry, add_widget_tx, state);
                                        Button::new(label(LUCIDE_CHECK).with_color(if valid {
                                            element_primary()
                                        } else {
                                            element_disabled()
                                        }))
                                        .with_style(ButtonStyle::hidden())
                                        .on_click({
                                            to_owned!(add_widget_tx);
                                            move |_| {
                                                if let (Some(new_key), Some(new_value)) =
                                                    new_entry.lock_ref().clone()
                                                {
                                                    let _ = add_widget_tx.send(None);
                                                    state.write_mut(|v| {
                                                        v.insert(new_key, new_value)
                                                    });
                                                }
                                            }
                                        })
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

                            let editor =
                                row((key_editor, value_editor, abort, SignalWidget::new(confirm)));

                            let _ = add_widget_tx.send(Some(Box::new(editor)));
                        }
                    });

                Box::new(new_button)
            })
        });
        Box::new(col((
            StreamWidget::new(values),
            StreamWidget::new(new_entry),
        )))
    }

    fn create_editor_project<S: 'static + Send + Sync + Clone + StateProjected<Item = Self>>(
        state: S,
        assets: &AssetCache,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Self::create_editor(state.project_ref(|v| v, |v| v), assets)
    }
}

// impl<T> Editable for Box<T>
// where
//     T: Editable,
// {
//     const INLINE: bool = true;

//     fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
//         state: S,
//     ) -> Box<dyn Send + Widget> {
//         T::create_editor(state.map_value(|v| *v, |v| Box::new(v)))
//     }

//     fn create_editor_project<S: 'static + Send + Sync + StateStreamRef<Item = Self> + StateWrite>(
//         state: S,
//     ) -> Box<dyn Send + Widget>
//     where
//         Self: Sized,
//     {
//         T::create_editor_project(Arc::new(state.project_ref(|v| &**v, |v| &mut **v)))
//     }
// }

register_editable!(String, f32, i32, Entity, Vec2, Vec3, Quat);

#[doc(hidden)]
pub mod __private {
    pub use futures;
    pub use inventory;
    pub use violet;
}
