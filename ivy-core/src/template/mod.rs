use flax::{Entity, EntityBuilder};
use futures::{future::BoxFuture, FutureExt};
use ivy_assets::{loadable::ResourceDesc, AssetCache};

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

/// Offline bundle descriptor
pub trait BundleDesc:
    Clone + ResourceDesc<Error = anyhow::Error> + serde::Serialize + serde::de::DeserializeOwned
where
    <Self as ResourceDesc>::Output: Bundle + 'static,
{
}

pub trait BundleDescDyn: 'static + Send + Sync {
    fn load_dyn<'a>(
        &'a self,
        assets: &'a AssetCache,
    ) -> BoxFuture<'a, anyhow::Result<Box<dyn Bundle>>>;
}

impl<T> BundleDescDyn for T
where
    T: BundleDesc,
    T::Output: Bundle + 'static,
{
    fn load_dyn<'a>(
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
pub struct TemplateDesc {
    bundles: Vec<Box<dyn BundleDescDyn>>,
}

impl TemplateDesc {
    pub fn new() -> Self {
        Self {
            bundles: Vec::new(),
        }
    }

    pub fn with_bundle<B: 'static + BundleDescDyn>(mut self, bundle: B) -> Self {
        self.bundles.push(Box::new(bundle));
        self
    }
}

impl ResourceDesc for TemplateDesc {
    type Output = Template;
    type Error = anyhow::Error;

    async fn load(&self, assets: &AssetCache) -> Result<Self::Output, Self::Error> {
        let mut bundles = Vec::new();
        for bundle in &self.bundles {
            let loaded_bundle = bundle.load_dyn(assets).await?;
            bundles.push(loaded_bundle);
        }
        Ok(Template { bundles })
    }
}
