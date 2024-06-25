use std::{hash::Hash, io, path::Path};

use crate::{
    service::{FileSystemMapService, FsAssetError},
    Asset, AssetCache, AssetDesc,
};

impl<V, P> AssetDesc<V> for P
where
    Path: AssetDesc<V>,
    P: 'static + Send + Sync + Eq + Hash + AsRef<Path> + Clone + std::fmt::Debug,
    V: 'static + Send + Sync,
{
    type Error = <Path as AssetDesc<V>>::Error;

    fn load(&self, assets: &AssetCache) -> Result<Asset<V>, Self::Error> {
        assets.try_load(self.as_ref())
    }
}

/// Helper trait for assets that can load directly from a path
pub trait AssetFromPathExt {
    type Error: 'static + From<FsAssetError>;

    fn load_from_path(path: &Path, assets: &AssetCache) -> Result<Asset<Self>, Self::Error>;
}

impl<V> AssetDesc<V> for Path
where
    V: AssetFromPathExt,
{
    type Error = V::Error;

    fn load(&self, assets: &AssetCache) -> Result<Asset<V>, Self::Error> {
        V::load_from_path(self, assets)
    }
}

impl AssetFromPathExt for Vec<u8> {
    type Error = FsAssetError;

    fn load_from_path(path: &Path, assets: &AssetCache) -> Result<Asset<Self>, Self::Error> {
        Ok(assets.insert(assets.service::<FileSystemMapService>().load_bytes(path)?))
    }
}
