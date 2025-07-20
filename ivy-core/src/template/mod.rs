use std::{
    any::Any,
    collections::BTreeMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Weak,
    },
    task::{Context, Waker},
};

use downcast_rs::{impl_downcast, DowncastSync};
use flax::{Entity, EntityBuilder};
use futures::{future::BoxFuture, FutureExt, StreamExt};
use glam::Vec2;
use itertools::Itertools;
use ivy_assets::{
    declare_resource,
    loadable::{Loadable, LoadableDyn},
    AssetCache, Resource,
};
use ivy_editable::{register_editable, registry::EDITABLE_REGISTRY, Editable};
use palette::Srgba;
use parking_lot::{Mutex, RwLock};
use violet::{
    core::{
        layout::Align,
        state::{StateExt, StateSink, StateStream, StateStreamRef, StateWrite},
        style::{element_warning, surface_tertiary, SizeExt, StyleExt},
        to_owned,
        widget::{
            bold, card, col, interactive::select_list::SelectList, label, raised_card, row, Button,
            ButtonStyle, Collapsible, Rectangle, ScrollArea, StreamWidget,
        },
        Scope, Widget,
    },
    futures_signals::signal::{Mutable, SignalExt},
    lucide::icons::{LUCIDE_PACKAGE, LUCIDE_TRASH_2},
};

use crate::{
    bundle::Bundle,
    bundle_registry::{BundleRegistration, BUNDLE_REGISTRY},
};

/// Defines an entity template to construct an entity using [[Bundle]]s
pub struct Template {
    bundles: Vec<Box<dyn Bundle>>,
}

impl Default for Template {
    fn default() -> Self {
        Self::new()
    }
}

impl Template {
    pub fn new() -> Self {
        Self {
            bundles: Vec::new(),
        }
    }

    pub fn with_bundle<B: 'static + Bundle>(mut self, bundle: B) -> Self {
        self.bundles.push(Box::new(bundle));
        self
    }

    pub fn build(&self) -> EntityBuilder {
        let mut entity = Entity::builder();
        for bundle in &self.bundles {
            bundle.mount(&mut entity);
        }
        entity
    }
}

pub trait BundleDescDyn: LoadableDyn {
    fn load_as_bundle<'a>(
        &'a self,
        assets: &'a AssetCache,
    ) -> BoxFuture<'a, anyhow::Result<Box<dyn Bundle>>>;

    fn clone_bundle(&self) -> Box<dyn BundleDesc>;

    fn tag_name(&self) -> &'static str;
    fn as_sync_any_mut(&mut self) -> &mut (dyn Send + Sync + Any);
    fn as_sync_any(&self) -> &(dyn Send + Sync + Any);
}

/// Offline bundle descriptor
pub trait BundleDesc: 'static + Send + Sync + BundleDescDyn + DowncastSync {}

impl_downcast!(BundleDesc);

impl<T> BundleDescDyn for T
where
    T: BundleDesc + Loadable + Clone,
    T::Output: Resource + Bundle + 'static,
{
    fn load_as_bundle<'a>(
        &'a self,
        assets: &'a AssetCache,
    ) -> BoxFuture<'a, anyhow::Result<Box<dyn Bundle>>> {
        async move {
            match Loadable::load(self, assets).await {
                Ok(bundle) => Ok(Box::new(bundle) as Box<dyn Bundle>),
                Err(e) => Err(e),
            }
        }
        .boxed()
    }

    fn tag_name(&self) -> &'static str {
        <T::Output as Resource>::tag_name()
    }

    fn clone_bundle(&self) -> Box<dyn BundleDesc> {
        Box::new(self.clone())
    }

    fn as_sync_any_mut(&mut self) -> &mut (dyn Send + Sync + Any) {
        self
    }

    fn as_sync_any(&self) -> &(dyn Send + Sync + Any) {
        self
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
struct ErasedBundleDesc {
    bundle: Box<dyn BundleDesc>,
}

impl ErasedBundleDesc {
    fn new(bundle: Box<dyn BundleDesc>) -> Self {
        Self { bundle }
    }

    fn editor<S: 'static + Send + Sync + StateStreamRef<Item = Self> + StateWrite>(
        &self,
        state: S,
    ) -> Box<dyn Widget + Send> {
        let editor = EDITABLE_REGISTRY.get_by_type((*self.bundle).type_id());

        match editor {
            Some(editor) => {
                let editor = (editor.create_editor_projected)(Box::new(
                    state.project_ref(|v| v.bundle.as_sync_any(), |v| v.bundle.as_sync_any_mut()),
                ));

                Box::new(editor)
            }
            None => Box::new(label(self.bundle.tag_name())),
        }
    }
}

impl std::fmt::Debug for ErasedBundleDesc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ErasedBundleDesc")
            .field("bundle", &self.bundle.tag_name())
            .finish()
    }
}

impl Clone for ErasedBundleDesc {
    fn clone(&self) -> Self {
        Self {
            bundle: self.bundle.clone_bundle(),
        }
    }
}

/// Offline descriptor of a template that is serializable
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct TemplateDesc {
    bundles: Vec<ErasedBundleDesc>,
}

impl Default for TemplateDesc {
    fn default() -> Self {
        Self::new()
    }
}

impl TemplateDesc {
    pub fn new() -> Self {
        Self {
            bundles: Vec::new(),
        }
    }

    pub fn with_bundle<B: 'static + BundleDesc>(mut self, bundle: B) -> Self {
        self.bundles.push(ErasedBundleDesc::new(Box::new(bundle)));
        self
    }
}

impl Editable for TemplateDesc {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + violet::core::state::StateDuplex<Item = Self>>(
        state: S,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        Self::create_editor_project(Arc::new(state.memo(TemplateDesc {
            bundles: Vec::new(),
        })))
    }
    fn create_editor_project<
        S: 'static + Send + Sync + Clone + StateStreamRef<Item = Self> + StateWrite,
    >(
        state: S,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        let bundles = state
            .clone()
            .project_ref(|v| &v.bundles, |v| &mut v.bundles);

        let editors = move |scope: &mut Scope| {
            let deduped = bundles
                .stream()
                .scan(None as Option<Vec<_>>, |state, item| {
                    let emit = match state {
                        Some(prev) if prev.len() == item.len() => None,
                        _ => {
                            *state = Some(item.clone());
                            Some(item)
                        }
                    };
                    futures::future::ready(emit)
                });

            // Create initial editors
            scope.spawn_stream(deduped, {
                move |scope, values| {
                    tracing::info!("Creating editors");

                    scope.detach_all();

                    values.iter().enumerate().for_each(|(i, bundle)| {
                        let item_state = bundles
                            .clone()
                            .project_ref(move |v| &v[i], move |v| &mut v[i]);

                        let state = Mutable::new(bundle.clone());

                        scope.spawn(state.signal_cloned().for_each(move |new_value| {
                            item_state.send(new_value);
                            async {}
                        }));

                        let text = bundle.bundle.tag_name();
                        to_owned!(bundles);
                        let discard = Button::label(LUCIDE_TRASH_2)
                            .with_style(ButtonStyle::hidden())
                            .with_tooltip_text("Remove Bundle")
                            .on_click(move |_| {
                                bundles.write_mut(|v| v.remove(i));
                            });

                        scope.attach(
                            card(Collapsible::new(
                                row((
                                    bold(text),
                                    Rectangle::new(Srgba::new(0.0, 0.0, 0.0, 0.0))
                                        .with_maximize(Vec2::X),
                                    discard,
                                ))
                                .with_cross_align(Align::Center),
                                bundle.editor(state),
                            ))
                            .with_background(surface_tertiary()),
                        );
                    });

                    col(()).with_stretch(true).mount(scope);
                }
            });
        };

        let (add_tx, add_rx) = flume::unbounded::<Option<Box<dyn Send + Widget>>>();
        // Create initial editors
        add_tx.send(None).ok();

        let add_new = move || {
            to_owned!(add_tx, state);
            Button::label("Add Bundle")
                .with_maximize(Vec2::X)
                .with_tooltip_text("Add new bundle")
                .on_click(move |_| {
                    to_owned!(add_tx);
                    let widget = BundleCreationWidget {
                        on_add: Box::new({
                            to_owned!(add_tx, state);
                            move |new_bundle| {
                                add_tx.send(None).ok();
                                tracing::info!("Writing to bundle");
                                if let Some(new_bundle) = new_bundle {
                                    state.write_mut(|v| v.bundles.push(new_bundle));
                                }
                            }
                        }),
                    };

                    let _ = add_tx.send(Some(Box::new(widget)));
                })
        };

        Box::new(
            col((
                ScrollArea::vertical(editors),
                StreamWidget::new(
                    add_rx
                        .into_stream()
                        .map(move |v| v.unwrap_or_else(|| Box::new(add_new()))),
                ),
            ))
            .with_cross_align(Align::Center)
            .with_stretch(true),
        )
    }
}

struct BundleCreationWidget {
    on_add: Box<dyn Fn(Option<ErasedBundleDesc>) + Send + Sync>,
}

impl Widget for BundleCreationWidget {
    fn mount(self, scope: &mut Scope<'_>) {
        let on_add = scope.store(self.on_add);
        let available_bundles = BUNDLE_REGISTRY
            .bundles()
            .values()
            .map(|v| BundleEntry { registration: *v })
            .collect_vec();

        #[derive(Clone, Copy)]
        struct BundleEntry {
            registration: BundleRegistration,
        }

        impl Widget for BundleEntry {
            fn mount(self, scope: &mut Scope<'_>) {
                row((label(LUCIDE_PACKAGE), label((self.registration.tag)()))).mount(scope)
            }
        }

        let selected = Mutable::new(None as Option<usize>);

        let value_editor = selected.stream_ref({
            to_owned!(available_bundles);
            move |i| {
                let &Some(i) = i else {
                    return None;
                };

                let entry = available_bundles[i];
                let editor = EDITABLE_REGISTRY.get_by_type((entry.registration.desc_type_id)());

                let value = Mutable::new(None as Option<ErasedBundleDesc>);
                let upcast = entry.registration.upcast_any;

                let add_controls = value.stream().map(move |v| {
                    let add = match v {
                        Some(bundle) => {
                            let bundle = bundle.clone();
                            let widget = Button::label("Add")
                                .success()
                                .on_click(move |scope| (scope.read(on_add)(Some(bundle.clone()))));

                            widget
                        }
                        None => Button::label("Add")
                            .disabled()
                            .with_tooltip_text("Missing fields"),
                    };

                    row((
                        add.with_maximize(Vec2::X),
                        Button::label("Cancel")
                            .with_maximize(Vec2::X)
                            .on_click(move |scope| scope.read(on_add)(None)),
                    ))
                });

                let widget = if let Some(editor) = editor {
                    Box::new(editor.create_editor(Box::new(
                        value.clone().lower_option().map_value(
                            |v| v.bundle.into_any_sync(),
                            move |v| ErasedBundleDesc::new(upcast(v)),
                        ),
                    ))) as Box<dyn Send + Widget>
                } else {
                    Box::new(
                        label("No editor available for this bundle").with_color(element_warning()),
                    )
                };

                Some(col((StreamWidget::new(add_controls), widget)).with_stretch(true))
            }
        });

        let selection_widget = raised_card(ScrollArea::vertical(SelectList::new(
            selected,
            available_bundles.clone(),
        )));

        col((
            selection_widget,
            raised_card(StreamWidget::new(value_editor)),
        ))
        .with_stretch(true)
        .mount(scope);
    }
}

declare_resource!(Template, TemplateDesc);

impl Loadable for TemplateDesc {
    type Output = Template;

    async fn load(&self, assets: &AssetCache) -> Result<Self::Output, anyhow::Error> {
        let mut bundles = Vec::new();
        for bundle in &self.bundles {
            let loaded_bundle = bundle.bundle.load_as_bundle(assets).await?;
            bundles.push(loaded_bundle);
        }
        Ok(Template { bundles })
    }
}

register_editable!(TemplateDesc);
