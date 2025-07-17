use std::{ffi::OsStr, marker::PhantomData, path::PathBuf};

use derivative::Derivative;
use futures::future::BoxFuture;

use crate::{
    loadable::{LoadFromPath, Loadable, Resource, ResourceDyn},
    service::FsAssetError,
    Asset, AssetCache, AsyncAssetDesc, AsyncAssetExt,
};

/// Describes an asset loaded from a relative filesystem path
#[derive(Derivative)]
#[derivative(Clone, Debug = "transparent", Hash, PartialEq, Eq, PartialOrd, Ord)]
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct AssetPath<T> {
    path: PathBuf,
    #[derivative(Debug = "ignore")]
    #[serde(skip)]
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

    pub async fn load_file_content(&self, assets: &AssetCache) -> Result<Vec<u8>, FsAssetError> {
        assets
            .service::<crate::service::FileSystemMapService>()
            .load_bytes_async(&self.path)
            .await
    }

    pub fn path_mut(&mut self) -> &mut PathBuf {
        &mut self.path
    }
}

impl<T> AsyncAssetDesc for AssetPath<T>
where
    T: LoadFromPath,
{
    type Output = T;

    type Error = anyhow::Error;

    async fn create(&self, assets: &AssetCache) -> Result<Asset<Self::Output>, Self::Error> {
        Ok(assets.insert(T::load_from_file(self.clone(), assets).await?))
    }

    fn label(&self) -> String {
        if let Some(filename) = self.path.file_name() {
            format!("{}({})", tynm::type_name::<T>(), filename.to_string_lossy())
        } else {
            format!("{}({})", tynm::type_name::<T>(), self.path().display())
        }
    }
}

impl<T: LoadFromPath> Resource for Asset<T> {
    type Desc = AssetPath<T>;

    fn tag_name() -> &'static str {
        "Asset"
    }
}

impl<T: LoadFromPath> Loadable for AssetPath<T> {
    type Output = Asset<T>;

    async fn load(&self, assets: &AssetCache) -> anyhow::Result<Self::Output> {
        Ok(AsyncAssetExt::load_async(self, assets).await?)
    }
}

trait AssetPathExt {
    fn load_dyn(&self, assets: &AssetCache) -> BoxFuture<anyhow::Result<Box<dyn ResourceDyn>>>;
}

impl<T: LoadFromPath> AssetPathExt for AssetPath<T> {
    fn load_dyn(&self, assets: &AssetCache) -> BoxFuture<anyhow::Result<Box<dyn ResourceDyn>>> {
        todo!()
    }
}
