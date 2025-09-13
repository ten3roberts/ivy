use std::{
    ffi::OsStr,
    marker::PhantomData,
    path::{Path, PathBuf},
};

use derivative::Derivative;
use futures::future::BoxFuture;

use crate::{
    hotreload::FileReloadService,
    loadable::{LoadFromPath, Loadable, Resource, ResourceDyn},
    service::{FileSystemMapService, FsAssetError},
    Asset, AssetCache, AsyncAssetExt, AsyncAssetKey,
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

// impl<T, P: Into<PathBuf>> From<P> for AssetPath<T> {
//     fn from(value: P) -> Self {
//         Self::new(value)
//     }
// }

impl<T> AssetPath<T> {
    /// Construct a new asset path identifier from a path and a given asset root.
    pub fn from_root(root: impl AsRef<Path>, path: impl AsRef<Path>) -> Self {
        let root = root.as_ref().canonicalize().unwrap();
        let path = path.as_ref().canonicalize().unwrap();

        Self::new(path.strip_prefix(&root).unwrap_or(&path).to_path_buf())
    }

    pub fn new(path: impl Into<PathBuf>) -> Self {
        let path = path.into();
        assert!(path.is_relative(), "AssetPath {path:?} must be relative");

        Self {
            path,
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

impl<T> AsyncAssetKey for AssetPath<T>
where
    T: LoadFromPath,
{
    type Output = T;

    type Error = anyhow::Error;

    async fn create(&self, assets: &AssetCache) -> Result<Asset<Self::Output>, Self::Error> {
        if let Some(reload) = assets.try_get_service::<FileReloadService>() {
            let full_path = assets
                .service::<FileSystemMapService>()
                .get_system_path(self.path());

            if let Err(err) = reload.track_path(Self::new(full_path)) {
                tracing::warn!("Failed to track asset path for hot-reloading: {err}");
            }
        }
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
