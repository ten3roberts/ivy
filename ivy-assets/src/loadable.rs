use std::{collections::BTreeMap, future::Future};

use futures::{stream, StreamExt, TryStreamExt};
use serde::de::DeserializeOwned;

use crate::{Asset, AssetCache, AssetPath, AsyncAssetDesc};

/// Signifies a type is a endpoint of a resource loading chain.
///
/// Many `AsyncAssetDesc` implementations can load to the same type, but this allows a type to
/// prefer one asset implementation over another.
pub trait Resource: 'static + Send + Sync {
    type Desc;

    // fn load(
    //     desc: Self::Desc,
    //     assets: &AssetCache,
    // ) -> impl Send + Future<Output = Result<Self, anyhow::Error>>
    // where
    //     Self: Sized;
}

pub trait LoadFromPath: 'static + Send + Sync + Sized {
    fn load_from_file(
        path: AssetPath<Self>,
        assets: &AssetCache,
    ) -> impl Send + Future<Output = Result<Self, anyhow::Error>>
    where
        Self: Sized;
}

/// Generic loading mechanism.
///
/// Allow converting a plain-description type into a loaded resource.
///
/// Further implementations for Vec, Option, and BTreeMap are provided.
pub trait Loadable: 'static + Send + Sync {
    /// The type of the resource that this can load.
    type Output: 'static + Send + Sync;

    fn load(
        &self,
        assets: &AssetCache,
    ) -> impl Send + Future<Output = Result<Self::Output, anyhow::Error>>
    where
        Self: Sized;
}

impl<T> LoadFromPath for T
where
    T: Resource,
    T::Desc: DeserializeOwned,
{
    async fn load_from_file(path: AssetPath<Self>, assets: &AssetCache) -> anyhow::Result<Self>
    where
        Self: Sized,
    {
        todo!()
    }
}

impl<T> Loadable for Vec<T>
where
    T: Loadable,
{
    type Output = Vec<T::Output>;

    async fn load(&self, assets: &AssetCache) -> Result<Self::Output, anyhow::Error> {
        stream::iter(self)
            .then(|item| item.load(assets))
            .try_collect()
            .await
    }
}

impl<K, V> Loadable for BTreeMap<K, V>
where
    K: 'static + Send + Sync + Ord + Clone,
    V: Loadable,
{
    type Output = BTreeMap<K, V::Output>;

    async fn load(&self, assets: &AssetCache) -> Result<Self::Output, anyhow::Error> {
        stream::iter(self.iter())
            .then(|(k, v)| async move { Ok((k.clone(), v.load(assets).await?)) })
            .try_collect()
            .await
    }
}

impl<T> Loadable for Option<T>
where
    T: Loadable,
{
    type Output = Option<T::Output>;

    async fn load(&self, assets: &AssetCache) -> Result<Self::Output, anyhow::Error> {
        if let Some(val) = self {
            Ok(Some(val.load(assets).await?))
        } else {
            Ok(None)
        }
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
struct AssetMeta {
    pub ty: String,
}

#[derive(serde::Serialize, serde::Deserialize)]
struct AssetPayload<T> {
    meta: AssetMeta,
    desc: T,
}

// impl<T> Loadable for T
// where
//     T: AsyncAssetDesc,
// {
//     type Output = Asset<T::Output>;

//     async fn load(&self, assets: &AssetCache) -> Result<Self::Output, Self::Error> {
//         let v = assets.try_load_async(self).await?;

//         Ok(v)
//     }
// }

// #[cfg(feature = "serde")]
// impl<T> LoadFromPath for T
// where
//     T: SerializableResource,
// {

//     async fn load(path: AssetPath<Self>, assets: &AssetCache) -> Result<Self, Self::Error> {
//         let content = assets
//             .service::<crate::service::FileSystemMapService>()
//             .load_bytes_async(path.path())
//             .await?;

//         let payload: AssetPayload<T::Desc> = serde_json::from_slice(&content[..])?;

//         if payload.meta.ty != T::tag_name() {
//             return Err(anyhow::anyhow!(
//                 "Asset type mismatch: expected {}, found {}",
//                 payload.meta.ty,
//                 T::tag_name()
//             ));
//         }

//         let asset = T::load(payload, assets).await?;
//         Ok(asset)
//     }
// }
