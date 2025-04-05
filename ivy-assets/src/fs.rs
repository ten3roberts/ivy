use std::{ffi::OsStr, fmt::Debug, marker::PhantomData, path::PathBuf};

use derivative::Derivative;

use crate::{
    loadable::ResourceFromPath,
    service::{FileSystemMapService, FsAssetError},
    AssetCache,
};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct BytesFromPath(pub PathBuf);

impl BytesFromPath {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self(path.into())
    }
}

impl ResourceFromPath for Vec<u8> {
    type Error = FsAssetError;

    async fn load(path: AssetPath<Self>, assets: &AssetCache) -> Result<Self, Self::Error> {
        assets
            .service::<FileSystemMapService>()
            .load_bytes_async(path.path())
            .await
    }
}

impl ResourceFromPath for String {
    type Error = FsAssetError;

    async fn load(path: AssetPath<Self>, assets: &AssetCache) -> Result<Self, Self::Error> {
        assets
            .service::<FileSystemMapService>()
            .load_string_async(path.path())
            .await
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
