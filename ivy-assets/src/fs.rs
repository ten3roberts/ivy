use std::{
    ffi::OsStr,
    fmt::Debug,
    marker::PhantomData,
    path::{Path, PathBuf},
};

use derivative::Derivative;
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
pub trait AsyncAssetFromPath: 'static + Send + Sync {
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

    fn create(&self, assets: &AssetCache) -> Result<Asset<V>, Self::Error> {
        V::load_from_path(self.as_ref(), assets)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BytesFromPath(pub PathBuf);

impl BytesFromPath {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }
}

impl AsyncAssetDesc for BytesFromPath {
    type Output = Vec<u8>;

    type Error = FsAssetError;

    fn create(
        &self,
        assets: &AssetCache,
    ) -> impl Future<Output = Result<Asset<Vec<u8>>, Self::Error>> + Send {
        <Vec<u8> as AsyncAssetFromPath>::load_from_path(self.0.as_ref(), assets)
    }
}

#[derive(Derivative)]
#[derivative(Clone, Debug = "transparent", Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct AssetPath<T> {
    path: PathBuf,
    #[derivative(Debug = "ignore")]
    #[cfg_attr(feature = "serde", serde(skip))]
    _marker: PhantomData<T>,
}

impl<T, P: Into<PathBuf>> From<P> for AssetPath<T> {
    fn from(value: P) -> Self {
        Self::new(value)
    }
}

impl<T> AssetPath<T> {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            _marker: PhantomData,
        }
    }

    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    pub fn file_name(&self) -> Option<&OsStr> {
        self.path.file_name()
    }
}

impl<T: AsyncAssetFromPath> AsyncAssetDesc for AssetPath<T> {
    type Output = T;

    type Error = T::Error;

    fn create(
        &self,
        assets: &AssetCache,
    ) -> impl Future<Output = Result<Asset<T>, Self::Error>> + Send {
        <T as AsyncAssetFromPath>::load_from_path(self.path.as_ref(), assets)
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

    async fn load_from_path(path: &Path, assets: &AssetCache) -> Result<Asset<Self>, Self::Error> {
        Ok(assets.insert(
            assets
                .service::<FileSystemMapService>()
                .load_bytes_async(path)
                .await?,
        ))
    }
}

impl AsyncAssetFromPath for String {
    type Error = FsAssetError;

    async fn load_from_path(path: &Path, assets: &AssetCache) -> Result<Asset<Self>, Self::Error> {
        Ok(assets.insert(
            assets
                .service::<FileSystemMapService>()
                .load_string_async(path)
                .await?,
        ))
    }
}

impl AssetFromPath for String {
    type Error = FsAssetError;

    fn load_from_path(path: &Path, assets: &AssetCache) -> Result<Asset<Self>, Self::Error> {
        Ok(assets.insert(assets.service::<FileSystemMapService>().load_string(path)?))
    }
}
