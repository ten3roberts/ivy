use std::{
    any::{Any, TypeId},
    borrow::Borrow,
    collections::HashMap,
    fmt::{Debug, Display},
    future::Future,
    hash::Hash,
    ops::Deref,
    path::Path,
    sync::Arc,
    task::Poll,
    time::Instant,
};

use dashmap::DashMap;

pub mod cell;
pub mod fs;
mod handle;
pub mod map;
#[cfg(feature = "serde")]
pub mod serde;
pub mod service;
pub mod stored;
use fs::{AssetFromPath, AsyncAssetFromPath};
use futures::{
    future::{BoxFuture, Shared, WeakShared},
    FutureExt, TryFutureExt,
};
pub use handle::Asset;
use image::DynamicImage;
use parking_lot::{RwLock, RwLockReadGuard};
use service::Service;

use self::{cell::AssetCell, handle::WeakHandle};

slotmap::new_key_type! {
    pub struct AssetId;
}

/// Storage of immutable assets.
///
/// Assets are accessed loaded through keys which are used to load the assets if not present.
///
// TODO: asset reload through immutable publish
#[derive(Clone)]
pub struct AssetCache {
    inner: Arc<AssetCacheInner>,
}

impl Debug for AssetCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssetCache").finish()
    }
}

#[derive(Debug)]
pub struct SharedError<E>(Arc<E>);

impl<E: Display> Display for SharedError<E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl<E: Debug + Display> std::error::Error for SharedError<E> {}

impl<E> Clone for SharedError<E> {
    #[cold]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

type KeyMap<K, V> = DashMap<K, WeakHandle<V>>;
type PendingKeyMap<K, V> = DashMap<
    <K as StoredKey>::Stored,
    WeakShared<BoxFuture<'static, Result<Asset<V>, SharedError<<K as AsyncAssetDesc<V>>::Error>>>>,
>;

/// Stores assets which are accessible through handles
struct AssetCacheInner {
    pending_keys: DashMap<TypeId, Box<dyn Any + Send + Sync>>,
    keys: DashMap<TypeId, Box<dyn Any + Send + Sync>>,
    cells: DashMap<TypeId, Box<dyn Any + Send + Sync>>,
    services: RwLock<HashMap<TypeId, Box<dyn Service + Send>>>,
}

impl AssetCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(AssetCacheInner {
                keys: DashMap::new(),
                cells: DashMap::new(),
                services: Default::default(),
                pending_keys: DashMap::new(),
            }),
        }
    }

    pub fn try_load<K, V>(&self, desc: &K) -> Result<Asset<V>, K::Error>
    where
        K: ?Sized + AssetDesc<V>,
        V: 'static + Send + Sync,
    {
        ivy_profiling::profile_function!(format!("{desc:?}"));

        let _span = tracing::debug_span!("AssetCache::try_load", key = std::any::type_name::<K>())
            .entered();
        if let Some(handle) = self.get(desc) {
            return Ok(handle);
        }

        // Load the asset and insert it to get a handle
        let value = desc.create(self)?;

        self.inner
            .keys
            .entry(TypeId::of::<(K::Stored, V)>())
            .or_insert_with(|| Box::<KeyMap<K::Stored, V>>::default())
            .downcast_mut::<KeyMap<K::Stored, V>>()
            .unwrap()
            .insert(desc.to_stored(), value.downgrade());

        Ok(value)
    }

    #[track_caller]
    pub fn load<V>(&self, key: &(impl AssetDesc<V> + ?Sized)) -> Asset<V>
    where
        V: 'static + Send + Sync,
    {
        match self.try_load(key) {
            Ok(v) => v,
            Err(err) => {
                panic!("{err:?}");
            }
        }
    }

    pub fn try_load_async<K, V>(&self, desc: &K) -> AssetLoadFuture<V, K::Error>
    where
        K: ?Sized + AsyncAssetDesc<V>,
        V: 'static + Send + Sync,
    {
        // ivy_profiling::profile_function!(format!("{desc:?}"));

        if let Some(handle) = self.get_async(desc) {
            return AssetLoadFuture {
                inner: Ok(Ok(handle)),
            };
        }

        {
            let pending = self.inner.pending_keys.get(&TypeId::of::<(K::Stored, V)>());
            if let Some(pending) = pending {
                let pending = pending.downcast_ref::<PendingKeyMap<K, V>>().unwrap();

                if let Some(fut) = pending.get(desc).and_then(|v| WeakShared::upgrade(&v)) {
                    return AssetLoadFuture { inner: Err(fut) };
                }
            }
        }

        // Load the asset and insert it to get a handle
        let assets = self.clone();
        let stored = desc.to_stored();
        let desc_debug = format!("{desc:?}");
        let desc = desc.to_stored();

        let fut = async move {
            let start = Instant::now();
            let assets = assets;
            let value = desc
                .borrow()
                .create(&assets)
                .await
                .map_err(|v| SharedError(Arc::new(v)))?;

            tracing::info!(duration=?start.elapsed(), desc=%desc_debug, "loaded asset");

            assets
                .inner
                .keys
                .entry(TypeId::of::<(K::Stored, V)>())
                .or_insert_with(|| Box::<KeyMap<K::Stored, V>>::default())
                .downcast_mut::<KeyMap<K::Stored, V>>()
                .unwrap()
                .insert(desc, value.downgrade());

            Ok(value)
        }
        .boxed()
        .shared();

        {
            let mut pending = self
                .inner
                .pending_keys
                .entry(TypeId::of::<(K::Stored, V)>())
                .or_insert_with(|| Box::new(PendingKeyMap::<K, V>::new()));

            let pending = pending.downcast_mut::<PendingKeyMap<K, V>>().unwrap();
            pending.insert(stored, fut.downgrade().unwrap());
        }

        async_std::task::spawn(fut.clone());

        AssetLoadFuture { inner: Err(fut) }
    }

    pub async fn load_async<V>(&self, key: &(impl AsyncAssetDesc<V> + ?Sized)) -> Asset<V>
    where
        V: 'static + Send + Sync,
    {
        match self.try_load_async(key).await {
            Ok(v) => v,
            Err(err) => {
                let err = err.0;
                panic!("{err:?}");
            }
        }
    }
    pub fn get<K, V>(&self, key: &K) -> Option<Asset<V>>
    where
        K: ?Sized + AssetDesc<V>,
        V: 'static + Send + Sync,
    {
        // Keys of K
        let keys = self.inner.keys.get(&TypeId::of::<(K::Stored, V)>())?;

        let handle = keys
            .downcast_ref::<KeyMap<K::Stored, V>>()
            .unwrap()
            .get(key)?
            .upgrade()?;

        Some(handle)
    }

    pub fn get_async<K, V>(&self, key: &K) -> Option<Asset<V>>
    where
        K: ?Sized + AsyncAssetDesc<V>,
        V: 'static + Send + Sync,
    {
        // Keys of K
        let keys = self.inner.keys.get(&TypeId::of::<(K::Stored, V)>())?;

        let handle = keys
            .downcast_ref::<KeyMap<K::Stored, V>>()
            .unwrap()
            .get(key)?
            .upgrade()?;

        Some(handle)
    }

    /// Insert an asset without an associated key.
    ///
    /// This can be used for unique generated assets which can not be reproduced.
    pub fn insert<V: 'static + Send + Sync>(&self, value: V) -> Asset<V> {
        self.inner
            .cells
            .entry(TypeId::of::<V>())
            .or_insert_with(|| Box::new(AssetCell::<V>::new()))
            .downcast_mut::<AssetCell<V>>()
            .unwrap()
            .insert(value)
    }

    pub fn register_service<S: Service>(&self, service: S) {
        self.inner
            .services
            .write()
            .insert(TypeId::of::<S>(), Box::new(service));
    }

    pub fn service<S: Service>(&self) -> impl Deref<Target = S> + '_ + Send {
        RwLockReadGuard::map(self.inner.services.read(), |v| {
            v.get(&TypeId::of::<S>())
                .expect("Service not found")
                .as_any()
                .downcast_ref::<S>()
                .expect("Service type mismatch")
        })
    }
}

impl Default for AssetCache {
    fn default() -> Self {
        Self::new()
    }
}

pub trait StoredKey: 'static + Send + Sync + Hash + Eq {
    type Stored: 'static + Send + Sync + Hash + Eq + Borrow<Self>;
    fn to_stored(&self) -> Self::Stored;
}

impl<K> StoredKey for K
where
    K: 'static + Send + Sync + ?Sized + Hash + Eq + ToOwned,
    K::Owned: 'static + Send + Sync + Hash + Eq,
{
    type Stored = K::Owned;

    fn to_stored(&self) -> Self::Stored {
        self.to_owned()
    }
}

pub trait DynAssetDesc<V>: 'static + Send + Sync {
    fn load(&self, assets: &AssetCache) -> Asset<V>;
    fn try_load(&self, assets: &AssetCache) -> anyhow::Result<Asset<V>>;
}

impl<T, V> DynAssetDesc<V> for T
where
    T: AssetDesc<V>,
    T::Error: Into<anyhow::Error>,
    V: 'static + Send + Sync,
{
    fn load(&self, assets: &AssetCache) -> Asset<V> {
        assets.load(self)
    }

    fn try_load(&self, assets: &AssetCache) -> anyhow::Result<Asset<V>> {
        assets.try_load(self).map_err(Into::into)
    }
}

pub trait DynAsyncAssetDesc<V>: 'static + Send + Sync {
    fn load_async(&self, assets: &AssetCache) -> AssetLoadFuture<V, anyhow::Error>;
}

impl<T, V> DynAsyncAssetDesc<V> for T
where
    T: AsyncAssetDesc<V>,
    T::Error: Debug + Display,
    V: 'static + Send + Sync,
{
    fn load_async(&self, assets: &AssetCache) -> AssetLoadFuture<V, anyhow::Error> {
        let fut = assets.try_load_async(self);
        let inner = match fut.inner {
            Ok(v) => Ok(v.map_err(|v| SharedError(Arc::new(anyhow::Error::from(v))))),
            Err(fut) => Err(fut
                .map_err(|v| SharedError(Arc::new(anyhow::Error::from(v))))
                .boxed()
                .shared()),
        };

        AssetLoadFuture { inner }
    }
}

/// A key or descriptor which can be used to load an asset.
///
/// This trait is implemented for `Path`, `str` and `String` by default to load assets from the
/// filesystem using the provided [`FsProvider`].
pub trait AssetDesc<V>: StoredKey + Debug {
    type Error: 'static + Debug;

    fn create(&self, assets: &AssetCache) -> Result<Asset<V>, Self::Error>;
}

pub trait AsyncAssetDesc<V>: StoredKey + Debug + Send + Sync {
    type Error: Send + Sync + 'static + Debug;

    fn create(
        &self,
        assets: &AssetCache,
    ) -> impl Future<Output = Result<Asset<V>, Self::Error>> + Send;
}

impl AssetFromPath for DynamicImage {
    type Error = anyhow::Error;

    fn load_from_path(path: &Path, assets: &AssetCache) -> anyhow::Result<Asset<Self>> {
        let format = image::ImageFormat::from_path(path)?;
        let data = assets.try_load::<_, Vec<u8>>(path)?;
        let image = image::load_from_memory_with_format(&data, format)?;
        Ok(assets.insert(image))
    }
}

impl AsyncAssetFromPath for DynamicImage {
    type Error = anyhow::Error;

    async fn load_from_path(path: &Path, assets: &AssetCache) -> anyhow::Result<Asset<Self>> {
        let format = image::ImageFormat::from_path(path)?;
        let data = assets.try_load_async::<_, Vec<u8>>(path).await?;
        let image = async_std::task::spawn_blocking(move || {
            image::load_from_memory_with_format(&data, format)
        })
        .await?;
        Ok(assets.insert(image))
    }
}

type SharedLoadFuture<T, E> = Shared<BoxFuture<'static, Result<Asset<T>, SharedError<E>>>>;

pub struct AssetLoadFuture<T, E> {
    inner: Result<Result<Asset<T>, SharedError<E>>, SharedLoadFuture<T, E>>,
}

impl<T, E> AssetLoadFuture<T, E> {
    /// Returns the value if loaded
    /// Can be called multiple times until loaded
    pub fn try_get(&self) -> Option<Result<Asset<T>, SharedError<E>>> {
        match &self.inner {
            Ok(v) => Some(v.clone()),
            Err(v) => v.peek().cloned(),
        }
    }
}

impl<T, E> Future for AssetLoadFuture<T, E> {
    type Output = Result<Asset<T>, SharedError<E>>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match &mut self.inner {
            Ok(v) => Poll::Ready(v.clone()),
            Err(v) => v.poll_unpin(cx),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{convert::Infallible, path::Path};

    use crate::service::FsAssetError;

    use super::*;

    #[test]
    fn asset_cache() {
        impl AssetFromPath for () {
            type Error = FsAssetError;

            fn load_from_path(
                path: &Path,
                assets: &AssetCache,
            ) -> Result<Asset<Self>, Self::Error> {
                eprintln!("Loading {:?}", path);
                Ok(assets.insert(()))
            }
        }

        let assets = AssetCache::new();

        let content: Asset<()> = assets.load(&"Foo");
        let content2: Asset<()> = assets.load(&"Foo".to_string());
        let bar: Asset<()> = assets.load(&"Bar".to_string());
        let content4: Asset<()> = assets.load(&"Foo");

        assert_eq!(content, content2);

        assert!(Arc::ptr_eq(content.as_arc(), content2.as_arc()));
        assert!(!Arc::ptr_eq(content.as_arc(), bar.as_arc()));
        assert_eq!(content, content4);

        assert!(assets.get::<_, ()>(&"Bar".to_string()).is_some());

        drop(bar);

        assert!(assets.get::<_, ()>(&"Bar".to_string()).is_none());
    }

    #[test]
    fn async_load() {
        eprintln!("Starting async_load");
        struct YieldOnce {
            yielded: bool,
        }

        impl Future for YieldOnce {
            type Output = ();

            fn poll(
                mut self: std::pin::Pin<&mut Self>,
                cx: &mut std::task::Context<'_>,
            ) -> std::task::Poll<()> {
                if self.yielded {
                    std::task::Poll::Ready(())
                } else {
                    self.yielded = true;
                    cx.waker().wake_by_ref();
                    std::task::Poll::Pending
                }
            }
        }

        impl AsyncAssetDesc<()> for str {
            type Error = Infallible;

            async fn create(&self, assets: &AssetCache) -> Result<Asset<()>, Infallible> {
                eprintln!("Loading {:?}", self);
                YieldOnce { yielded: false }.await;

                eprintln!("Finished {:?}", self);
                Ok(assets.insert(()))
            }
        }

        let assets = AssetCache::new();

        eprintln!("Starting first request");
        let mut pending1 = Box::pin(assets.try_load_async::<_, ()>("Foo"));

        eprintln!("Starting second request");

        let pending2 = assets.try_load_async::<_, ()>("Foo");

        use futures::future::FutureExt;
        assert!((&mut pending1).now_or_never().is_none());
        let content2 = pending2.now_or_never().unwrap().unwrap();
        let content = pending1.now_or_never().unwrap().unwrap();
        assert!(Arc::ptr_eq(content.as_arc(), content2.as_arc()));

        let pending3 = assets
            .try_load_async::<_, ()>("Foo")
            .now_or_never()
            .unwrap()
            .unwrap();
        assert!(Arc::ptr_eq(content.as_arc(), pending3.as_arc()));
    }
}
