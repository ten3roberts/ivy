use std::any;

use facet::Facet;
use flax::{Entity, EntityBuilder};
use futures::{future::BoxFuture, FutureExt};
use ivy_assets::{loadable::Loadable, AssetCache, Resource};

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

trait LoadableBundle {
    fn load_dyn<'a>(
        &'a self,
        assets: &'a AssetCache,
    ) -> BoxFuture<'a, anyhow::Result<Box<dyn Bundle>>>;
}

/// Offline bundle descriptor
#[typetag::serde(tag = "type")]
pub trait BundleDesc: 'static + Send + Sync + LoadableBundle {}

impl<T> LoadableBundle for T
where
    T: 'static + Loadable,
    T::Output: Bundle + 'static,
{
    fn load_dyn<'a>(
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
}

/// Offline descriptor of a template that is serializable
pub struct TemplateDesc {
    bundles: Vec<Box<dyn BundleDesc>>,
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

impl Resource for Template {
    type Desc = TemplateDesc;
}

impl Loadable for TemplateDesc {
    type Output = Template;

    async fn load(&self, assets: &AssetCache) -> Result<Self::Output, anyhow::Error> {
        let mut bundles = Vec::new();
        for bundle in &self.bundles {
            let loaded_bundle = bundle.load_dyn(assets).await?;
            bundles.push(loaded_bundle);
        }
        Ok(Template { bundles })
    }
}
