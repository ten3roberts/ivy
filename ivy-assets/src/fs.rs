use std::{hash::Hash, path::Path};

use crate::{Asset, AssetCache, AssetKey};

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
