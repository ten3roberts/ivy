use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

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

    pub fn load_bytes(&self, path: impl AsRef<Path>) -> Result<Vec<u8>, FsAssetError> {
        let path = path.as_ref();
        let inner = || {
            let mut file = File::open(self.root.join(path))?;
            let mut bytes = Vec::new();
            file.read_to_end(&mut bytes)?;
            Ok(bytes)
        };

        inner().map_err(|err| FsAssetError {
            path: path.into(),
            error: err,
        })
    }

    pub fn load_string(&self, path: impl AsRef<Path>) -> Result<String, FsAssetError> {
        let path = path.as_ref();
        let inner = || {
            let mut file = File::open(self.root.join(path))?;
            let mut string = String::new();
            file.read_to_string(&mut string)?;
            Ok(string)
        };

        inner().map_err(|err| FsAssetError {
            path: path.into(),
            error: err,
        })
    }
}
