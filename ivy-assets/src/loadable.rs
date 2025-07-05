use std::{collections::BTreeMap, future::Future};

use futures::{stream, StreamExt, TryStreamExt};

use crate::{Asset, AssetCache, AssetPath, AsyncAssetDesc};

/// Generic loading mechanism.
///
/// Allow converting a plain-description type into a loaded resource.
///
/// Further implementations for Vec, Option, and BTreeMap are provided.
pub trait ResourceDesc: 'static + Send + Sync + Sized {
    type Output: Send + Sync;
    type Error: Send + Sync;

    fn load(
        &self,
        assets: &AssetCache,
    ) -> impl Send + Future<Output = Result<Self::Output, Self::Error>>;
}

/// Signifies a type is a endpoint of a resource loading chain.
///
/// Many `AsyncAssetDesc` implementations can load to the same type, but this allows a type to
/// prefer one asset implementation over another.
pub trait Resource: 'static + Send + Sync {
    type Desc: 'static + Send + Sync;

    fn load(
        desc: Self::Desc,
        assets: &AssetCache,
    ) -> impl Send + Future<Output = Result<Self, anyhow::Error>>
    where
        Self: Sized;
}

pub trait Loadable: 'static + Send + Sync {
    /// The type of the resource that this can load.
    type Output: Resource;
}

/// Base type for any resource that is serializeable from disk.
///
/// Serialized assets have a plain-data json representation, along with metadata.
///
/// Implementing this type will make [`AssetPath<Self>`] an Asset
pub trait SerializableResource: 'static + Send + Sync {
    /// Representation of the resource as serialized data. This data subsequently loads to `Self`
    type Desc: ResourceDesc + serde::Serialize + serde::de::DeserializeOwned;

    fn tag_name() -> String;
    fn load(
        payload: AssetPayload<Self::Desc>,
        assets: &AssetCache,
    ) -> impl Send + Future<Output = Result<Self, anyhow::Error>>
    where
        Self: Sized;
}

/// SerializableResources load from a file on the filesystem.
impl<T: SerializableResource> Resource for T {
    type Desc = AssetPath<T>;

    async fn load(path: AssetPath<T>, assets: &AssetCache) -> Result<Self, anyhow::Error>
    where
        Self: Sized,
    {
        let content = path.load_file_content(assets).await?;

        let payload: AssetPayload<T::Desc> = serde_json::from_slice(&content[..])?;

        if payload.meta.ty != T::tag_name() {
            return Err(anyhow::anyhow!(
                "Asset type mismatch: expected {}, found {}",
                payload.meta.ty,
                T::tag_name()
            ));
        }

        let asset = T::load(payload, assets).await?;
        Ok(asset)
    }
}

// Cached version
impl<T> Resource for Asset<T>
where
    T: Resource<Desc = AssetPath<T>>,
{
    type Desc = AssetPath<T>;

    async fn load(desc: Self::Desc, assets: &AssetCache) -> Result<Self, anyhow::Error>
    where
        Self: Sized,
    {
        let asset = assets.try_load_async(&desc).await?;

        Ok(asset)
    }
}

impl<T> ResourceDesc for AssetPath<T>
where
    T: Resource,
{
    type Output = Asset<T>;

    type Error = anyhow::Error;

    async fn load(&self, assets: &AssetCache) -> Result<Self::Output, Self::Error> {
        todo!()
    }
}

// /// Consumed by [`AssetPath`] and signifies that this resource can be loaded from the filesystem as
// /// an *Asset*.
// pub trait LoadFromPath: 'static + Send + Sync + Sized {
//     type Error: Send + Sync;

//     fn load(
//         path: AssetPath<Self>,
//         assets: &AssetCache,
//     ) -> impl Send + Future<Output = Result<Self, Self::Error>>;
// }

impl<T> ResourceDesc for Vec<T>
where
    T: ResourceDesc,
{
    type Output = Vec<T::Output>;

    type Error = T::Error;

    async fn load(&self, assets: &AssetCache) -> Result<Self::Output, Self::Error> {
        stream::iter(self)
            .then(|item| item.load(assets))
            .try_collect()
            .await
    }
}

impl<K, V> ResourceDesc for BTreeMap<K, V>
where
    K: 'static + Send + Sync + Ord + Clone,
    V: ResourceDesc,
{
    type Output = BTreeMap<K, V::Output>;

    type Error = V::Error;

    async fn load(&self, assets: &AssetCache) -> Result<Self::Output, Self::Error> {
        stream::iter(self.iter())
            .then(|(k, v)| async move { Ok((k.clone(), v.load(assets).await?)) })
            .try_collect()
            .await
    }
}

impl<T> ResourceDesc for Option<T>
where
    T: ResourceDesc,
{
    type Output = Option<T::Output>;

    type Error = T::Error;

    async fn load(&self, assets: &AssetCache) -> Result<Self::Output, Self::Error> {
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

// impl<T> ResourceDesc for T
// where
//     T: AsyncAssetDesc,
// {
//     type Output = Asset<T::Output>;

//     type Error = anyhow::Error;

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
//     type Error = anyhow::Error;

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
