use flax::{Entity, EntityBuilder};
use futures::{future::BoxFuture, FutureExt};
use ivy_assets::{loadable::ResourceDesc, AssetCache};
use serde::Deserialize;

use crate::bundle::Bundle;

/// Defines an entity template to construct an entity using [[Bundle]]s
pub struct Template {
    bundles: Vec<Box<dyn Send + Sync + Bundle>>,
}

impl Template {
    pub fn new() -> Self {
        Self {
            bundles: Vec::new(),
        }
    }

    pub fn with_bundle<B: 'static + Send + Sync + Bundle>(mut self, bundle: B) -> Self {
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

/// Offline bundle describtor
pub trait BundleDesc:
    Clone + ResourceDesc<Error = anyhow::Error> + serde::Serialize + serde::de::DeserializeOwned
where
    <Self as ResourceDesc>::Output: Bundle + 'static,
{
}

pub trait BundleDescDyn {
    fn load<'a>(&'a self, assets: &'a AssetCache)
        -> BoxFuture<'a, anyhow::Result<Box<dyn Bundle>>>;
}

impl<T> BundleDescDyn for T
where
    T: BundleDesc,
    T::Output: Bundle + 'static,
{
    fn load<'a>(
        &'a self,
        assets: &'a AssetCache,
    ) -> BoxFuture<'a, anyhow::Result<Box<dyn Bundle>>> {
        async move {
            match ResourceDesc::load(self, assets).await {
                Ok(bundle) => Ok(Box::new(bundle) as Box<dyn Bundle>),
                Err(e) => Err(e),
            }
        }
        .boxed()
    }
}

/// Offline descriptor of a template that is serializable
pub struct TemplateDesc {}
