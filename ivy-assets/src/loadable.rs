use std::{any::Any, collections::BTreeMap, future::Future, path::PathBuf};

use downcast_rs::{impl_downcast, Downcast, DowncastSync};
use futures::{future::BoxFuture, stream, FutureExt, StreamExt, TryStreamExt};
use serde::de::DeserializeOwned;

use crate::{
    meta::{AssetMeta, AssetPayload},
    AssetCache, AssetPath,
};

/// Signifies a type is a endpoint of a resource loading chain.
///
/// Many `AsyncAssetDesc` implementations can load to the same type, but this allows a type to
/// prefer one asset implementation over another.
pub trait Resource: 'static + Send + Sync {
    type Desc: LoadablePayload<Output = Self>;

    fn tag_name() -> &'static str;
}

pub trait ResourceDyn: Downcast {}

pub trait LoadableDyn: 'static + Send + Sync + DowncastSync {
    fn load_dyn(
        &self,
        meta: AssetMeta,
        path: PathBuf,
        assets: &AssetCache,
    ) -> BoxFuture<anyhow::Result<Box<dyn ResourceDyn>>>;
    fn upcast_boxed_any(&self) -> fn(Box<dyn Send + Sync + Any>) -> Box<dyn LoadableDyn>;

    fn clone_dyn(&self) -> Box<dyn LoadableDyn>;
}

impl<T> ResourceDyn for T where T: Resource {}

impl<T> LoadableDyn for T
where
    T: Clone + LoadablePayload,
    T::Output: ResourceDyn,
{
    fn load_dyn(
        &self,
        meta: AssetMeta,
        path: PathBuf,
        assets: &AssetCache,
    ) -> BoxFuture<anyhow::Result<Box<dyn ResourceDyn>>> {
        let assets = assets.clone();
        async move {
            let resource = self.load(meta, AssetPath::new(path), &assets).await?;
            Ok(Box::new(resource) as Box<dyn ResourceDyn>)
        }
        .boxed()
    }

    fn clone_dyn(&self) -> Box<dyn LoadableDyn> {
        Box::new(self.clone())
    }

    fn upcast_boxed_any(&self) -> fn(Box<dyn Send + Sync + Any>) -> Box<dyn LoadableDyn> {
        move |value: Box<dyn Send + Sync + Any>| value.downcast::<T>().expect("Failed to downcast")
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

    fn resource_name() -> Option<&'static str>;

    fn extensions() -> &'static [&'static str];
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

pub trait LoadablePayload: 'static + Send + Sync {
    /// The type of the resource that this can load.
    type Output: 'static + Send + Sync;

    fn load(
        &self,
        meta: AssetMeta,
        path: AssetPath<Self::Output>,
        assets: &AssetCache,
    ) -> impl Send + Future<Output = Result<Self::Output, anyhow::Error>>
    where
        Self: Sized;
}

impl<T: 'static + Loadable> LoadablePayload for T {
    type Output = T::Output;

    async fn load(
        &self,
        _meta: AssetMeta,
        _path: AssetPath<Self::Output>,
        assets: &AssetCache,
    ) -> Result<Self::Output, anyhow::Error>
    where
        Self: Sized,
    {
        self.load(assets).await
    }
}

async fn load_asset_payload<T>(
    path: &AssetPath<T>,
    assets: &AssetCache,
) -> anyhow::Result<AssetPayload<T::Desc>>
where
    T: Resource,
    T::Desc: DeserializeOwned,
{
    let content = assets
        .service::<crate::service::FileSystemMapService>()
        .load_bytes_async(path.path())
        .await?;

    let payload: AssetPayload<T::Desc> = serde_json::from_slice(&content[..])?;

    if payload.meta.type_name != T::tag_name() {
        return Err(anyhow::anyhow!(
            "Asset type mismatch: expected {}, found {}",
            payload.meta.type_name,
            T::tag_name()
        ));
    }

    Ok(payload)
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
        let payload = load_asset_payload(&path, assets).await?;
        let asset = payload.desc.load(payload.meta, path, assets).await?;
        Ok(asset)
    }

    fn resource_name() -> Option<&'static str> {
        Some(T::tag_name())
    }

    fn extensions() -> &'static [&'static str] {
        &["asset"]
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
