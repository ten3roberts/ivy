use std::{future::ready, marker::Sized, sync::Arc};

use flax::{component::ComponentValue, entity_ids, Entity, Query};
use futures::{channel::oneshot, FutureExt, StreamExt};
use glam::{Vec2, Vec3};
use itertools::Itertools;
use ivy_ui::{
    streamed::StreamedUiExt,
    violet::{
        self,
        core::{
            layout::Align,
            state::{StateDuplex, StateExt, StateMut, StateStream},
            style::SizeExt,
            to_owned,
            unit::Unit,
            widget::{
                card, col,
                interactive::{
                    overlay::{overlay_state, Overlay},
                    select_list::SelectList,
                },
                label, row, Button, FutureWidget, InputBox, SignalWidget, StreamWidget,
            },
            Scope, Widget,
        },
        futures_signals::signal::{Mutable, SignalExt},
    },
};

use crate::register_editable;

/// A trait for components that can be edited in the editor.
pub trait Editable: 'static + Send + Sync {
    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        state: S,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized;
}

impl Editable for String {
    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        value: S,
    ) -> Box<dyn Send + Widget> {
        Box::new(InputBox::new(value))
    }
}

impl Editable for i32 {
    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        value: S,
    ) -> Box<dyn Send + Widget> {
        Box::new(InputBox::new(value))
    }
}

impl Editable for f32 {
    fn create_editor<S: 'static + Send + Sync + StateDuplex<Item = Self>>(
        value: S,
    ) -> Box<dyn Send + Widget> {
        Box::new(InputBox::new(value))
    }
}

impl Editable for Vec2 {
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

impl<T> Editable for Vec<T>
where
    T: ComponentValue + Editable + Clone,
{
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
                            Button::label("-").on_click(move |_| {
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
                    let new_button = Button::label("Add").on_click({
                        to_owned!(value);
                        move |_| {
                            to_owned!(value);
                            let new_value = Mutable::new(None as Option<T>);

                            let editor = T::create_editor(new_value.clone().lower_option());
                            let confirm = Button::label("Confirm").on_click({
                                to_owned!(add_widget_tx);
                                move |_| {
                                    if let Some(new_value) = new_value.lock_ref().clone() {
                                        let _ = add_widget_tx.send(None);
                                        value.write_mut(|v| v.push(new_value));
                                    }
                                }
                            });
                            let editor = row((editor, confirm));

                            let _ = add_widget_tx.send(Some(Box::new(editor)));
                        }
                    });

                    Box::new(new_button)
                })
            })),
        )))
    }
}

register_editable!(String, f32, i32, Entity, Vec2, Vec3);
