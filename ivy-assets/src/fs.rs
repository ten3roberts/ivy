use std::{ffi::OsStr, fmt::Debug, marker::PhantomData, path::PathBuf};

use derivative::Derivative;

use crate::{
    loadable::ResourceFromPath,
    service::{FileSystemMapService, FsAssetError},
    AssetCache, AssetPath,
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
