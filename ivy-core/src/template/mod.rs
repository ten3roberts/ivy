use std::any;

use facet::Facet;
use flax::{Entity, EntityBuilder};
use futures::{future::BoxFuture, FutureExt};
use ivy_assets::{
    declare_resource,
    loadable::{Loadable, LoadableDyn},
    AssetCache, Resource,
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

trait LoadableBundle: LoadableDyn {
    fn load_as_bundle<'a>(
        &'a self,
        assets: &'a AssetCache,
    ) -> BoxFuture<'a, anyhow::Result<Box<dyn Bundle>>>;

    fn clone_bundle(&self) -> Box<dyn BundleDesc>;
}

/// Offline bundle descriptor
#[typetag::serde(tag = "type")]
pub trait BundleDesc: 'static + Send + Sync + LoadableBundle {}

impl<T> LoadableBundle for T
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

    fn clone_bundle(&self) -> Box<dyn BundleDesc> {
        Box::new(self.clone())
    }
}

/// Offline descriptor of a template that is serializable
#[derive(serde::Serialize, serde::Deserialize)]
pub struct TemplateDesc {
    bundles: Vec<Box<dyn BundleDesc>>,
}

impl Clone for TemplateDesc {
    fn clone(&self) -> Self {
        Self {
            bundles: self.bundles.iter().map(|v| v.clone_bundle()).collect(),
        }
    }
}

impl TemplateDesc {
    pub fn new() -> Self {
        Self {
            bundles: Vec::new(),
        }
    }

    pub fn with_bundle<B: 'static + BundleDesc>(mut self, bundle: B) -> Self {
        self.bundles.push(Box::new(bundle));
        self
    }
}

declare_resource!(Template, TemplateDesc);

impl Loadable for TemplateDesc {
    type Output = Template;

    async fn load(&self, assets: &AssetCache) -> Result<Self::Output, anyhow::Error> {
        let mut bundles = Vec::new();
        for bundle in &self.bundles {
            let loaded_bundle = bundle.load_as_bundle(assets).await?;
            bundles.push(loaded_bundle);
        }
        Ok(Template { bundles })
    }
}
