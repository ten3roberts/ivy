use std::{collections::BTreeMap, future::Future};

use downcast_rs::{impl_downcast, Downcast, DowncastSync};
use futures::{future::BoxFuture, stream, FutureExt, StreamExt, TryStreamExt};
use serde::de::DeserializeOwned;

use crate::{meta::AssetPayload, AssetCache, AssetPath};

/// Signifies a type is a endpoint of a resource loading chain.
///
/// Many `AsyncAssetDesc` implementations can load to the same type, but this allows a type to
/// prefer one asset implementation over another.
pub trait Resource: 'static + Send + Sync {
    type Desc: Loadable<Output = Self>;

    fn type_name() -> &'static str;
}

pub trait ResourceDyn: Downcast {}

pub trait LoadableDyn: 'static + Send + Sync + DowncastSync {
    fn load_dyn(&self, assets: &AssetCache) -> BoxFuture<anyhow::Result<Box<dyn ResourceDyn>>>;
    fn clone_dyn(&self) -> Box<dyn LoadableDyn>;
}

impl<T> ResourceDyn for T where T: Resource {}

impl<T> LoadableDyn for T
where
    T: Clone + Loadable,
    T::Output: ResourceDyn,
{
    fn load_dyn(&self, assets: &AssetCache) -> BoxFuture<anyhow::Result<Box<dyn ResourceDyn>>> {
        let assets = assets.clone();
        async move {
            let resource = self.load(&assets).await?;
            Ok(Box::new(resource) as Box<dyn ResourceDyn>)
        }
        .boxed()
    }

    fn clone_dyn(&self) -> Box<dyn LoadableDyn> {
        Box::new(self.clone())
    }
}

impl_downcast!(ResourceDyn);
impl_downcast!(LoadableDyn);

pub trait LoadFromPath: 'static + Send + Sync + Sized {
    fn load_from_file(
        path: AssetPath<Self>,
        assets: &AssetCache,
    ) -> impl Send + Future<Output = Result<Self, anyhow::Error>>
    where
        Self: Sized;
}

/// Generic loading mechanism.
///
/// Allow converting a plain-description type into a loaded resource.
///
/// Further implementations for Vec, Option, and BTreeMap are provided.
pub trait Loadable: 'static + Send + Sync {
    /// The type of the resource that this can load.
    type Output: 'static + Send + Sync;

    fn load(
        &self,
        assets: &AssetCache,
    ) -> impl Send + Future<Output = Result<Self::Output, anyhow::Error>>
    where
        Self: Sized;
}

impl<T> LoadFromPath for T
where
    T: Resource,
    T::Desc: DeserializeOwned,
{
    async fn load_from_file(path: AssetPath<Self>, assets: &AssetCache) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        let content = assets
            .service::<crate::service::FileSystemMapService>()
            .load_bytes_async(path.path())
            .await?;

        let payload: AssetPayload<T::Desc> = serde_json::from_slice(&content[..])?;

        if payload.meta.type_name != T::type_name() {
            return Err(anyhow::anyhow!(
                "Asset type mismatch: expected {}, found {}",
                payload.meta.type_name,
                T::type_name()
            ));
        }

        let asset = payload.desc.load(assets).await?;
        Ok(asset)
    }
}

impl<T> Loadable for Vec<T>
where
    T: Loadable,
{
    type Output = Vec<T::Output>;

    async fn load(&self, assets: &AssetCache) -> Result<Self::Output, anyhow::Error> {
        stream::iter(self)
            .then(|item| item.load(assets))
            .try_collect()
            .await
    }
}

impl<K, V> Loadable for BTreeMap<K, V>
where
    K: 'static + Send + Sync + Ord + Clone,
    V: Loadable,
{
    type Output = BTreeMap<K, V::Output>;

    async fn load(&self, assets: &AssetCache) -> Result<Self::Output, anyhow::Error> {
        stream::iter(self.iter())
            .then(|(k, v)| async move { Ok((k.clone(), v.load(assets).await?)) })
            .try_collect()
            .await
    }
}

impl<T> Loadable for Option<T>
where
    T: Loadable,
{
    type Output = Option<T::Output>;

    async fn load(&self, assets: &AssetCache) -> Result<Self::Output, anyhow::Error> {
        if let Some(val) = self {
            Ok(Some(val.load(assets).await?))
        } else {
            Ok(None)
        }
    }
}
