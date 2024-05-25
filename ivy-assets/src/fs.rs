use std::{hash::Hash, io, path::Path};

use crate::{service::FileSystemMapService, Asset, AssetCache, AssetKey};

impl<V, P> AssetKey<V> for P
where
    Path: AssetKey<V>,
    P: 'static + Send + Sync + Eq + Hash + AsRef<Path> + Clone,
    V: 'static + Send + Sync,
{
    type Error = <Path as AssetKey<V>>::Error;

    fn load(&self, assets: &AssetCache) -> Result<Asset<V>, Self::Error> {
        assets.try_load(self.as_ref())
    }
}

pub trait AssetFromPathExt {
    type Error: 'static + From<io::Error>;

    fn load(path: &Path, assets: &AssetCache) -> Result<Asset<Self>, Self::Error>;
}

impl<V> AssetKey<V> for Path
where
    V: AssetFromPathExt,
{
    type Error = V::Error;

    fn load(&self, assets: &AssetCache) -> Result<Asset<V>, Self::Error> {
        V::load(self, assets)
    }
}

impl AssetFromPathExt for Vec<u8> {
    type Error = io::Error;

    fn load(path: &Path, assets: &AssetCache) -> Result<Asset<Self>, Self::Error> {
        Ok(assets.insert(assets.service::<FileSystemMapService>().load_bytes(path)?))
    }
}
// i
// impl<K> Loadable<K> for Bytes
// where
//     K: AssetKey,
//     K: AsRef<Path>,
// {
//     type Error = std::io::Error;

//     fn load(key: K, assets: &AssetCache) -> Result<Self, Self::Error>
//     where
//         Self: Sized,
//     {
//         Ok(std::fs::read(key.as_ref())?.into())
//     }
// }
