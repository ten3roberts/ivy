use std::{collections::BTreeMap, future::Future, path::Path};

use futures::{stream, StreamExt, TryStreamExt};

use crate::{
    fs::{AssetPath, AsyncAssetFromPath, BytesFromPath},
    Asset, AssetCache, AsyncAssetDesc,
};

/// Represents a type that can be loaded from an offline descriptor
pub trait ResourceDescriptor: 'static + Send + Sync
where
    Self: Sized,
{
    type Desc: Send + Sync;
}

pub trait LoadWithPath: 'static + Send + Sync {
    type Output: Send + Sync;
    type Error: Send + Sync;

    fn load_from_path(
        self,
        path: AssetPath<Self::Output>,
        assets: &AssetCache,
    ) -> impl Send + Future<Output = Result<Self::Output, Self::Error>>;
}

/// General trait for any type of asset/resource descriptor that can be loaded
pub trait Load: 'static + Send + Sync {
    type Output: Send + Sync;
    type Error: Send + Sync;

    fn load(
        self,
        assets: &AssetCache,
    ) -> impl Send + Future<Output = Result<Self::Output, Self::Error>>;
}

impl<T: Load> LoadWithPath for T {
    type Output = <Self as Load>::Output;

    type Error = <Self as Load>::Error;

    async fn load_from_path(
        self,
        _: AssetPath<Self::Output>,
        assets: &AssetCache,
    ) -> Result<Self::Output, Self::Error> {
        Load::load(self, assets).await
    }
}

impl<T> Load for Vec<T>
where
    T: Load,
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

impl<K, V> Load for BTreeMap<K, V>
where
    K: 'static + Send + Sync + Ord,
    V: Load,
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

impl<V> Load for Option<V>
where
    V: Load,
{
    type Output = Option<V::Output>;

    type Error = V::Error;

    async fn load(self, assets: &AssetCache) -> Result<Self::Output, Self::Error> {
        if let Some(val) = self {
            Ok(Some(val.load(assets).await?))
        } else {
            Ok(None)
        }
    }
}

#[cfg(feature = "serde")]
impl<V> AsyncAssetFromPath for V
where
    V: 'static + ResourceDescriptor + Send + Sync,
    V::Desc: serde::de::DeserializeOwned + LoadWithPath<Output = V>,
    <V::Desc as LoadWithPath>::Error: Into<anyhow::Error>,
{
    type Error = anyhow::Error;

    async fn load_from_path(path: &Path, assets: &AssetCache) -> Result<Asset<V>, Self::Error> {
        let content = assets.try_load_async(&BytesFromPath::new(path)).await?;
        let desc: V::Desc = serde_json::from_slice(&content[..])?;

        Ok(assets.insert(
            desc.load_from_path(AssetPath::new(path), assets)
                .await
                .map_err(Into::into)?,
        ))
    }
}

impl<V> Load for V
where
    V: 'static + AsyncAssetDesc + Send + Sync,
    V::Error: std::fmt::Debug + std::fmt::Display,
    // V::Desc: DeserializeOwned + LoadWithPath<Output = V>,
    // <V::Desc as LoadWithPath>::Error: Into<anyhow::Error>,
{
    type Output = Asset<<Self as AsyncAssetDesc>::Output>;
    type Error = anyhow::Error;

    async fn load(self, assets: &AssetCache) -> Result<Asset<V::Output>, Self::Error> {
        assets.try_load_async(&self).await.map_err(Into::into)
    }
}
