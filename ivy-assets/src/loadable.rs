use std::{future::Future, path::Path};

use serde::de::DeserializeOwned;

use crate::{
    fs::{AssetPath, AsyncAssetFromPath, BytesFromPath},
    Asset, AssetCache,
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

impl<V> AsyncAssetFromPath for V
where
    V: 'static + ResourceDescriptor + Send + Sync,
    V::Desc: DeserializeOwned + LoadWithPath<Output = V>,
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
