use std::{collections::BTreeMap, future::Future};

use futures::{stream, StreamExt, TryStreamExt};
use serde::de::DeserializeOwned;

use crate::{fs::AssetPath, Asset, AssetCache, AsyncAssetDesc};

pub trait Resource: 'static + Send + Sync + Sized {
    type Desc: ResourceDesc;
}

/// Represents an owned resource that needs to be initialized from its descriptor.
///
/// Resources can *access* and load assets, but are themselves not assets.
pub trait ResourceDesc: 'static + Send + Sync + Sized {
    type Output: Send + Sync;
    type Error: Send + Sync;

    fn load(
        self,
        assets: &AssetCache,
    ) -> impl Send + Future<Output = Result<Self::Output, Self::Error>>;
}

/// Allows a resource to load itself directly from a path, for example a png image.
///
/// Any implementation will also implement [`AsyncAssetDesc`] for `AssetPath<Self>`
pub trait ResourceFromPath: 'static + Send + Sync + Sized {
    type Error: Send + Sync;

    fn load(
        path: AssetPath<Self>,
        assets: &AssetCache,
    ) -> impl Send + Future<Output = Result<Self, Self::Error>>;
}

impl<T> ResourceDesc for Vec<T>
where
    T: ResourceDesc,
{
    type Output = Vec<T::Output>;

    type Error = T::Error;

    async fn load(self, assets: &AssetCache) -> Result<Self::Output, Self::Error> {
        stream::iter(self)
            .then(|item| item.load(assets))
            .try_collect()
            .await
    }
}

impl<K, V> ResourceDesc for BTreeMap<K, V>
where
    K: 'static + Send + Sync + Ord,
    V: ResourceDesc,
{
    type Output = BTreeMap<K, V::Output>;

    type Error = V::Error;

    async fn load(self, assets: &AssetCache) -> Result<Self::Output, Self::Error> {
        stream::iter(self)
            .then(|(k, v)| async move { Ok((k, v.load(assets).await?)) })
            .try_collect()
            .await
    }
}

impl<T> ResourceDesc for Option<T>
where
    T: ResourceDesc,
{
    type Output = Option<T::Output>;

    type Error = T::Error;

    async fn load(self, assets: &AssetCache) -> Result<Self::Output, Self::Error> {
        if let Some(val) = self {
            Ok(Some(val.load(assets).await?))
        } else {
            Ok(None)
        }
    }
}

impl<T> ResourceDesc for T
where
    T: AsyncAssetDesc,
{
    type Output = Asset<T::Output>;

    type Error = anyhow::Error;

    async fn load(self, assets: &AssetCache) -> Result<Self::Output, Self::Error> {
        let v = assets.try_load_async(&self).await?;

        Ok(v)
    }
}

impl<T> AsyncAssetDesc for AssetPath<T>
where
    T: ResourceFromPath,
    T::Error: Into<anyhow::Error>,
{
    type Output = T;

    type Error = anyhow::Error;

    async fn create(&self, assets: &AssetCache) -> Result<Asset<Self::Output>, Self::Error> {
        Ok(assets.insert(T::load(self.clone(), assets).await.map_err(Into::into)?))
    }
}

#[cfg(feature = "serde")]
impl<T> ResourceFromPath for T
where
    T: Resource,
    T::Desc: ResourceDesc<Output = T> + DeserializeOwned,
    <T::Desc as ResourceDesc>::Error: Into<anyhow::Error>,
{
    type Error = anyhow::Error;

    async fn load(path: AssetPath<Self>, assets: &AssetCache) -> Result<Self, Self::Error> {
        let content = assets
            .service::<crate::service::FileSystemMapService>()
            .load_bytes_async(path.path())
            .await?;

        let desc: T::Desc = serde_json::from_slice(&content[..])?;

        desc.load(assets).await.map_err(Into::into)
    }
}
