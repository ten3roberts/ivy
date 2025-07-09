use std::{
    any::{self, Any},
    sync::Arc,
};

use downcast_rs::{impl_downcast, DowncastSync};
use facet::Facet;
use flax::{Entity, EntityBuilder};
use futures::{future::BoxFuture, FutureExt};
use itertools::Itertools;
use ivy_assets::{
    declare_resource,
    loadable::{Loadable, LoadableDyn},
    AssetCache, Resource,
};
use ivy_editable::{register_editable, registry::EDITABLE_REGISTRY, Editable, Projection};
use violet::core::{
    state::{Project, StateDuplex, StateExt, StateMut, StateSink, StateStreamRef},
    style::surface_tertiary,
    to_owned,
    widget::{card, col, label, Collapsible, StreamWidget},
    Widget,
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

                Box::new(
                    card(Collapsible::label(self.bundle.typetag_name(), editor))
                        .with_background(surface_tertiary()),
                )
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

impl Editable for TemplateDesc {
    const INLINE: bool = true;

    fn create_editor<S: 'static + Send + Sync + violet::core::state::StateDuplex<Item = Self>>(
        state: S,
    ) -> Box<dyn Send + Widget>
    where
        Self: Sized,
    {
        // let bundles = state.transform(|v| v.bundles, |v, new_value| v.bundles = new_value);

        let bundles = Arc::new(
            state
                .map_value(|v| v.bundles, |v| TemplateDesc { bundles: v })
                .memo(Vec::new()),
        );

        let bundles_editor = bundles.clone().stream_ref(move |v| {
            to_owned!(bundles);
            let bundles = v
                .iter()
                .enumerate()
                .map(move |(i, bundle)| {
                    let item_state = bundles
                        .clone()
                        .project_ref(move |v| &v[i], move |v| &mut v[i]);
                    // .transform(|v| v[i].clone(), |v, new| v[i] = new);

                    bundle.editor(item_state)
                })
                .collect_vec();

            col(bundles).with_stretch(true)
        });

        Box::new(StreamWidget::new(bundles_editor))
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
