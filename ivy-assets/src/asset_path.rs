use std::{ffi::OsStr, future::Future, marker::PhantomData, path::PathBuf};

use derivative::Derivative;

use crate::{loadable::Resource, service::FsAssetError, Asset, AssetCache, AsyncAssetDesc};

/// Describes an asset loaded from a relative filesystem path
#[derive(Derivative)]
#[derivative(Clone, Debug = "transparent", Hash, PartialEq, Eq, PartialOrd, Ord)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct AssetPath<T>
where
    T: Resource,
{
    path: PathBuf,
    #[derivative(Debug = "ignore")]
    #[cfg_attr(feature = "serde", serde(skip))]
    _marker: PhantomData<T>,
}

impl<T: Resource, P: Into<PathBuf>> From<P> for AssetPath<T> {
    fn from(value: P) -> Self {
        Self::new(value)
    }
}

impl<T: Resource> AssetPath<T> {
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

    pub async fn load_file_content(&self, assets: &AssetCache) -> Result<Vec<u8>, FsAssetError> {
        assets
            .service::<crate::service::FileSystemMapService>()
            .load_bytes_async(&self.path)
            .await
    }
}

impl<T> AsyncAssetDesc for AssetPath<T>
where
    T: Resource<Desc = AssetPath<T>>,
{
    type Output = T;

    type Error = anyhow::Error;

    async fn create(&self, assets: &AssetCache) -> Result<Asset<Self::Output>, Self::Error> {
        Ok(assets.insert(T::load(self.clone(), assets).await?))
    }

    fn label(&self) -> String {
        if let Some(filename) = self.path().file_name() {
            format!("{}({})", tynm::type_name::<T>(), filename.to_string_lossy())
        } else {
            format!("{}({})", tynm::type_name::<T>(), self.path().display())
        }
    }
}
