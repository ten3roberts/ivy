use std::{
    convert::Infallible,
    fs::File,
    io::{self, BufReader},
    path::{Path, PathBuf},
};

use futures::AsyncReadExt;
use thiserror::Error;

/// A service is registered with the asset cache and is used to load assets.
pub trait Service: 'static + Send + Sync + Downcast {}

pub trait Downcast {
    fn as_any(&self) -> &dyn std::any::Any;
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any;
}

impl<T: Service> Downcast for T {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

#[derive(Debug, Error)]
#[error("Failed to load asset from {path:?}")]
pub struct FsAssetError {
    path: PathBuf,
    #[source]
    error: io::Error,
}

impl From<Infallible> for FsAssetError {
    fn from(_: Infallible) -> Self {
        unreachable!()
    }
}

/// Load assets from a configured asset root
pub struct FileSystemMapService {
    pub root: PathBuf,
}

impl Service for FileSystemMapService {}

impl Default for FileSystemMapService {
    fn default() -> Self {
        Self {
            root: PathBuf::from("assets"),
        }
    }
}

impl FileSystemMapService {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn load_reader(&self, path: impl AsRef<Path>) -> Result<BufReader<File>, FsAssetError> {
        let path = path.as_ref();

        let inner = || {
            let file = File::open(self.root.join(path))?;
            Ok(BufReader::new(file))
        };

        inner().map_err(|err| FsAssetError {
            path: path.into(),
            error: err,
        })
    }

    pub async fn load_bytes_async(
        &self,
        path: impl AsRef<Path> + Send,
    ) -> Result<Vec<u8>, FsAssetError> {
        let path = path.as_ref();
        let inner = async {
            let mut file = async_std::fs::File::open(self.root.join(path)).await?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes).await?;
            Ok(bytes)
        };

        inner.await.map_err(|err| FsAssetError {
            path: path.into(),
            error: err,
        })
    }

    pub async fn load_string_async(&self, path: impl AsRef<Path>) -> Result<String, FsAssetError> {
        let path = path.as_ref();
        let inner = async {
            let mut file = async_std::fs::File::open(self.root.join(path)).await?;
            let mut string = String::new();
            file.read_to_string(&mut string).await?;
            Ok(string)
        };

        inner.await.map_err(|err| FsAssetError {
            path: path.into(),
            error: err,
        })
    }
}
