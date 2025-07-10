use std::{
    any::{self, Any},
    sync::Arc,
};

use downcast_rs::{impl_downcast, DowncastSync};
use facet::Facet;
use flax::{Entity, EntityBuilder};
use futures::{
    future::{ready, BoxFuture},
    FutureExt, StreamExt,
};
use glam::Vec2;
use itertools::Itertools;
use ivy_assets::{
    declare_resource,
    loadable::{Loadable, LoadableDyn},
    AssetCache, Resource,
};
use ivy_editable::{register_editable, registry::EDITABLE_REGISTRY, Editable, Projection};
use palette::Srgba;
use violet::{
    core::{
        layout::Align,
        state::{
            Project, State, StateDuplex, StateExt, StateMut, StateRef, StateSink, StateStream,
            StateStreamRef,
        },
        style::{surface_tertiary, SizeExt, StyleExt},
        to_owned,
        widget::{
            bold, card, col, label, row, Button, ButtonStyle, Collapsible, Rectangle, StreamWidget,
        },
        Scope, Widget,
    },
    futures_signals::signal_vec::{MutableVec, SignalVecExt, VecDiff},
    lucide::icons::{LUCIDE_PLUS, LUCIDE_TRASH_2},
};

use crate::bundle::Bundle;

/// Defines an entity template to construct an entity using [[Bundle]]s
pub struct Template {
    bundles: Vec<Box<dyn Bundle>>,
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

trait ProjectedState: StateMut + StateStreamRef + StateSink {}

impl<T: ?Sized + StateMut + StateStreamRef + StateSink> ProjectedState for T {}

pub trait BundleDescDyn: LoadableDyn {
    fn load_as_bundle<'a>(
        &'a self,
        assets: &'a AssetCache,
    ) -> BoxFuture<'a, anyhow::Result<Box<dyn Bundle>>>;

    fn clone_bundle(&self) -> Box<dyn BundleDesc>;

    fn type_name(&self) -> &'static str;
    fn as_sync_any_mut(&mut self) -> &mut (dyn Send + Sync + Any);
    fn as_sync_any(&self) -> &(dyn Send + Sync + Any);
}

/// Offline bundle descriptor
#[typetag::serde]
pub trait BundleDesc: 'static + Send + Sync + BundleDescDyn + DowncastSync {}

impl_downcast!(BundleDesc);

impl<T> BundleDescDyn for T
where
    T: BundleDesc + Loadable + Clone,
    T::Output: Bundle + 'static,
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

    fn type_name(&self) -> &'static str {
        any::type_name::<T>()
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

    fn editor<S: 'static + Send + Sync + StateStreamRef<Item = Self> + StateMut>(
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
            None => Box::new(label(self.bundle.type_name())),
        }
    }
}

impl std::fmt::Debug for ErasedBundleDesc {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ErasedBundleDesc")
            .field("bundle", &self.bundle.typetag_name())
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

struct MutableVecItem<T> {
    inner: MutableVec<T>,
    index: usize,
}

impl<T> MutableVecItem<T> {
    fn new(inner: MutableVec<T>, index: usize) -> Self {
        Self { inner, index }
    }
}

impl<T> State for MutableVecItem<T> {
    type Item = T;
}

impl<T: 'static + Send + Sync + Clone> StateStreamRef for MutableVecItem<T> {
    fn stream_ref<F: 'static + Send + Sync + FnMut(&Self::Item) -> V, V: 'static + Send>(
        &self,
        mut func: F,
    ) -> impl 'static + Send + futures::Stream<Item = V>
    where
        Self: Sized,
    {
        let index = self.index;

        self.inner
            .signal_vec_cloned()
            .to_stream()
            .filter_map(move |diff| {
                let res = match diff {
                    VecDiff::Replace { values } => values.get(index).cloned(),
                    VecDiff::InsertAt { .. } => todo!(),
                    VecDiff::UpdateAt { index: at, value } => (at == index).then_some(value),
                    VecDiff::RemoveAt { .. } => todo!(),
                    VecDiff::Move { .. } => todo!(),
                    VecDiff::Push { .. } => todo!(),
                    VecDiff::Pop {} => todo!(),
                    VecDiff::Clear {} => todo!(),
                };

                futures::future::ready(res.map(|v| func(&v)))
            })
    }
}

impl<T: 'static + Send + Sync + Clone> StateRef for MutableVecItem<T> {
    fn read_ref<F: FnOnce(&Self::Item) -> V, V>(&self, f: F) -> V
    where
        Self: Sized,
    {
        todo!()
    }
}

impl<T: 'static + Send + Sync + Clone> StateMut for MutableVecItem<T> {
    fn write_mut<F: FnOnce(&mut Self::Item) -> V, V>(&self, f: F) -> V
    where
        Self: Sized,
    {
        let mut lock = self.inner.lock_mut();

        let mut value = lock[self.index].clone();
        let res = f(&mut value);
        lock.set_cloned(self.index, value);
        res
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
        let original_state = Arc::new(state);
        let bundles = Arc::new(
            original_state
                .clone()
                .map_value(|v| v.bundles, |v| TemplateDesc { bundles: v })
                .memo(Vec::new()),
        );

        let widget = move |scope: &mut Scope| {
            // let mut attached_editors = Vec::new();

            // Create initial editors

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

            scope.spawn_stream(deduped, move |scope, values| {
                values
                    .iter()
                    .enumerate()
                    .map(|(i, bundle)| {
                        let item_state = bundles
                            .clone()
                            .project_ref(move |v| &v[i], move |v| &mut v[i]);

                        let text = bundle.bundle.typetag_name();
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
                                bundle.editor(item_state),
                            ))
                            .with_background(surface_tertiary()),
                        )
                    })
                    .collect_vec();

                col(()).with_stretch(true).mount(scope);
            });
        };

        let add_new = Button::label("Add Bundle").with_tooltip_text("Add new bundle");
        Box::new(col((widget, add_new)).with_stretch(true))
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
