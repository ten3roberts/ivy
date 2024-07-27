use std::{fmt::Debug, path::Path};

use futures::Future;

use crate::{
    service::{FileSystemMapService, FsAssetError},
    Asset, AssetCache, AssetDesc, AsyncAssetDesc, StoredKey,
};

/// Helper trait for assets that can load directly from a path
pub trait AssetFromPath {
    type Error: 'static + From<FsAssetError> + Debug;

    fn load_from_path(path: &Path, assets: &AssetCache) -> Result<Asset<Self>, Self::Error>;
}

/// Supports loading an asset directly from a path
pub trait AsyncAssetFromPath {
    type Error: 'static + Send + Sync + From<FsAssetError> + Debug;

    fn load_from_path(
        path: &Path,
        assets: &AssetCache,
    ) -> impl Future<Output = Result<Asset<Self>, Self::Error>> + Send;
}

impl<P, V> AssetDesc<V> for P
where
    V: AssetFromPath,
    P: ?Sized + AsRef<Path> + StoredKey + Debug,
{
    type Error = V::Error;

    fn load(&self, assets: &AssetCache) -> Result<Asset<V>, Self::Error> {
        V::load_from_path(self.as_ref(), assets)
    }
}

impl<P, V> AsyncAssetDesc<V> for P
where
    V: AsyncAssetFromPath,
    P: ?Sized + AsRef<Path> + StoredKey + Debug,
{
    type Error = V::Error;

    fn create(
        &self,
        assets: &AssetCache,
    ) -> impl Future<Output = Result<Asset<V>, Self::Error>> + Send {
        V::load_from_path(self.as_ref(), assets)
    }
}

impl AssetFromPath for Vec<u8> {
    type Error = FsAssetError;

    fn load_from_path(path: &Path, assets: &AssetCache) -> Result<Asset<Self>, Self::Error> {
        Ok(assets.insert(assets.service::<FileSystemMapService>().load_bytes(path)?))
    }
}

impl AsyncAssetFromPath for Vec<u8> {
    type Error = FsAssetError;

    fn load_from_path(
        path: &Path,
        assets: &AssetCache,
    ) -> impl Future<Output = Result<Asset<Self>, Self::Error>> + Send {
        async move {
            Ok(assets.insert(
                assets
                    .service::<FileSystemMapService>()
                    .load_bytes_async(path)
                    .await?,
            ))
        }
    }
}

impl AssetFromPath for String {
    type Error = FsAssetError;

    fn load_from_path(path: &Path, assets: &AssetCache) -> Result<Asset<Self>, Self::Error> {
        Ok(assets.insert(assets.service::<FileSystemMapService>().load_string(path)?))
    }
}
