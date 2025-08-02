//! Sync and Async asset system and caching.
//!
//! Allows for async and blocking retrieval of assets.
//!
//! ## Asset
//! An asset is a *shared* value that can be loaded from an [`AssetDesc`] or [`AsyncAssetDesc`].
//!
//! Once loaded, the asset is retained, and further requests will reuse the same asset.
//!
//! The core purpose of an asset is to load data once, and then share it across the application.
//! ## Resource
//!
//! Unlike an asset, a resource is a common description for *any* type that can be loaded or processed from a descriptor.
//!
//! The value provided is not necessarily deduplicated.
//!
//! For instance, an image, texture, or 3D model is an *Asset*, as they are intended to be loaded
//! and shared and reused across the application.
//!
//! However, a vector of images is a *resource*, as it needs to be loaded from the descriptor (list
//! of paths), but the vector itself does not necessarily need to be shared or deduplicated.
//!
//! For convenience, `AsyncAssetDesc` also implements `ResourceDesc`, loading into an `Asset<V>`
use std::{
    any::{Any, TypeId},
    borrow::Borrow,
    collections::HashMap,
    error::Error,
    fmt::{Debug, Display},
    future::Future,
    hash::Hash,
    ops::Deref,
    sync::Arc,
    task::Poll,
    time::Duration,
};

use async_std::{path::PathBuf, task::sleep};
use dashmap::{
    mapref::one::{MappedRef, MappedRefMut},
    DashMap,
};

mod asset_path;
pub mod cell;
pub mod fs;
mod handle;
pub mod hotreload;
pub mod loadable;
pub mod map;
pub mod meta;
pub mod registry;
pub mod service;
pub mod stored;
pub mod timeline;
use futures::{
    future::{BoxFuture, Shared, WeakShared},
    FutureExt, TryFutureExt,
};
use futures_signals::signal::{Mutable, ReadOnlyMutable};
pub use handle::Asset;
use image::DynamicImage;
use ivy_profiling::profile_scope;
use parking_lot::{RwLock, RwLockReadGuard};
use service::{FileSystemMapService, Service};
use timeline::{AssetInfo, Timelines};

use crate::loadable::LoadFromPath;
pub use crate::loadable::Resource;

use self::{cell::AssetCell, handle::WeakAsset};

pub use asset_path::*;
pub use ivy_derive::Resource;

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
    span: Option<usize>,
}

impl Debug for AssetCache {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AssetCache").finish()
    }
}

pub struct SharedError(Arc<anyhow::Error>);

impl Display for SharedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Display::fmt(&self.0, f)
    }
}

impl std::error::Error for SharedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.0.source()
    }
}

impl Debug for SharedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        Debug::fmt(&self.0, f)
    }
}

impl Clone for SharedError {
    #[cold]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

type KeyMap<K, V> = DashMap<K, WeakAsset<V>>;
type PendingKeyMap<K, V> = DashMap<
    <K as StoredKey>::Stored,
    WeakShared<BoxFuture<'static, Result<Asset<V>, SharedError>>>,
>;

struct TypeMap {
    inner: DashMap<TypeId, Box<dyn Any + Send + Sync>>,
}

impl TypeMap {
    pub fn new() -> Self {
        Self {
            inner: DashMap::new(),
        }
    }

    pub fn insert<T: 'static + Send + Sync>(&self, value: T) {
        self.inner.insert(TypeId::of::<T>(), Box::new(value));
    }

    pub fn get<T: 'static + Send + Sync>(
        &self,
    ) -> std::option::Option<MappedRef<'_, TypeId, Box<dyn Any + Send + Sync>, T>> {
        self.inner.get(&TypeId::of::<T>()).map(|v| {
            dashmap::mapref::one::Ref::map(v, |v| v.downcast_ref::<T>().expect("Type mismatch"))
        })
    }

    pub fn entry<T: 'static + Send + Sync>(
        &self,
    ) -> dashmap::Entry<'_, TypeId, Box<dyn Any + Send + Sync>> {
        self.inner.entry(TypeId::of::<T>())
    }

    pub fn entry_or_default<T: 'static + Send + Sync + Default>(
        &self,
    ) -> MappedRefMut<'_, TypeId, Box<dyn Any + Send + Sync>, T> {
        match self.entry::<T>() {
            dashmap::Entry::Occupied(occupied_entry) => occupied_entry
                .into_ref()
                .map(|v| v.downcast_mut::<T>().expect("Type mismatch")),
            dashmap::Entry::Vacant(vacant_entry) => {
                let value = Box::new(T::default());
                vacant_entry
                    .insert(value)
                    .map(|v| v.downcast_mut::<T>().expect("Type mismatch"))
            }
        }
    }
}

/// Stores assets which are accessible through handles
struct AssetCacheInner {
    pending_keys: TypeMap,
    keys: TypeMap,
    cells: TypeMap,
    services: RwLock<HashMap<TypeId, Box<dyn Service + Send>>>,
    timelines: Mutable<Timelines>,
}

impl AssetCache {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(AssetCacheInner {
                keys: TypeMap::new(),
                cells: TypeMap::new(),
                pending_keys: TypeMap::new(),
                services: Default::default(),
                timelines: Mutable::new(Timelines::new()),
            }),
            span: None,
        }
    }

    pub fn try_load<K>(&self, desc: &K) -> Result<Asset<K::Output>, K::Error>
    where
        K: ?Sized + AssetDesc,
    {
        ivy_profiling::profile_function!(format!("{desc:?}"));

        let _span = tracing::debug_span!("AssetCache::try_load", key = std::any::type_name::<K>())
            .entered();

        if let Some(handle) = self.get(desc) {
            return Ok(handle);
        }

        const ENABLE_SYNC_SPANS: bool = false;

        let span_id = if ENABLE_SYNC_SPANS {
            let info = AssetInfo {
                name: desc.label(),
                asset_type: TypeId::of::<K::Output>(),
                type_name: tynm::type_name::<K::Output>(),
            };

            Some(self.inner.timelines.lock_mut().open_span(info, self.span))
        } else {
            self.span
        };

        // Load the asset and insert it to get a handle
        let value = desc.create(&Self {
            inner: self.inner.clone(),
            span: span_id,
        });

        if let Some(span_id) = span_id {
            if ENABLE_SYNC_SPANS {
                self.inner
                    .timelines
                    .lock_mut()
                    .close_span(span_id, value.as_ref().ok().map(|v| v.id()));
            }
        }

        let value = value?;

        self.inner
            .keys
            .entry_or_default::<KeyMap<K::Stored, K::Output>>()
            .insert(desc.to_stored(), value.downgrade());

        Ok(value)
    }

    #[track_caller]
    pub fn load<K>(&self, key: &K) -> Asset<K::Output>
    where
        K: AssetDesc,
    {
        match self.try_load(key) {
            Ok(v) => v,
            Err(err) => {
                panic!("{err:?}");
            }
        }
    }

    pub fn try_load_async<K>(&self, desc: &K) -> AssetLoadFuture<K::Output>
    where
        K: ?Sized + AsyncAssetKey,
    {
        if let Some(handle) = self.get_async(desc) {
            return AssetLoadFuture {
                inner: Ok(Ok(handle)),
            };
        }

        {
            let pending = self.inner.pending_keys.get::<PendingKeyMap<K, K::Output>>();

            if let Some(pending) = pending {
                if let Some(fut) = pending.get(desc).and_then(|v| WeakShared::upgrade(&v)) {
                    return AssetLoadFuture { inner: Err(fut) };
                }
            }
        }

        let info = AssetInfo {
            name: desc.label(),
            asset_type: TypeId::of::<K::Output>(),
            type_name: tynm::type_name::<K::Output>(),
        };
        let span_id = self.inner.timelines.lock_mut().open_span(info, self.span);

        let assets = Self {
            inner: self.inner.clone(),
            span: Some(span_id),
        };

        let stored = desc.to_stored();
        let desc = desc.to_stored();

        // Load the asset and insert it to get a handle
        let fut = async move {
            let assets = assets;
            let value: Result<Asset<K::Output>, _> = desc
                .borrow()
                .create(&assets)
                .await
                .map_err(|v| SharedError(Arc::new(v.into())));

            assets
                .inner
                .timelines
                .lock_mut()
                .close_span(span_id, value.as_ref().ok().map(|v| v.id()));

            let value = value?;

            let value2 = value.clone();
            async_std::task::spawn(async move {
                sleep(Duration::from_secs(1)).await;
                drop(value2);
            });

            assets
                .inner
                .keys
                .entry_or_default::<KeyMap<K::Stored, K::Output>>()
                .insert(desc, value.downgrade());

            Ok(value)
        }
        .boxed()
        .shared();

        {
            let pending = self
                .inner
                .pending_keys
                .entry_or_default::<PendingKeyMap<K, K::Output>>();

            pending.insert(stored, fut.downgrade().unwrap());
        }

        async_std::task::spawn(fut.clone());

        AssetLoadFuture { inner: Err(fut) }
    }

    pub async fn load_async<K: AsyncAssetKey + ?Sized>(&self, key: &K) -> Asset<K::Output> {
        match self.try_load_async(key).await {
            Ok(v) => v,
            Err(err) => {
                let err = err.0;
                panic!("{err:?}");
            }
        }
    }

    pub fn get<K>(&self, key: &K) -> Option<Asset<K::Output>>
    where
        K: ?Sized + AssetDesc,
    {
        // Keys of K
        let keys = self.inner.keys.get::<KeyMap<K::Stored, K::Output>>()?;

        let handle = keys.get(key)?.upgrade()?;

        Some(handle)
    }

    pub fn get_async<K>(&self, key: &K) -> Option<Asset<K::Output>>
    where
        K: ?Sized + AsyncAssetKey,
    {
        // Keys of K
        let keys = self.inner.keys.get::<KeyMap<K::Stored, K::Output>>()?;

        let handle = keys.get(key)?.upgrade()?;

        Some(handle)
    }

    /// Insert an asset without an associated key.
    ///
    /// This can be used for unique generated assets which can not be reproduced.
    pub fn insert<V: 'static + Send + Sync>(&self, value: V) -> Asset<V> {
        self.inner
            .cells
            .entry_or_default::<AssetCell<V>>()
            .insert(value)
    }

    pub fn reload<K: AsyncAssetKey>(&self, key: &K) -> AssetLoadFuture<K::Output> {
        // Remove the key from the pending keys
        let pending = self.inner.pending_keys.get::<PendingKeyMap<K, K::Output>>();
        if let Some(pending) = pending {
            pending.remove(key);
        }

        self.inner
            .keys
            .get::<KeyMap<K::Stored, K::Output>>()
            .and_then(|v| v.remove(key));

        // Try to load the asset again
        self.try_load_async(key)
    }

    pub fn register_service<S: Service>(&self, service: S) {
        service.register(self);
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

    pub fn try_get_service<S: Service>(&self) -> Option<impl Deref<Target = S> + '_ + Send> {
        RwLockReadGuard::try_map(self.inner.services.read(), |v| {
            Some(
                v.get(&TypeId::of::<S>())?
                    .as_any()
                    .downcast_ref::<S>()
                    .expect("Service type mismatch"),
            )
        })
        .ok()
    }

    /// Returns asset loading timelines
    pub fn timelines(&self) -> ReadOnlyMutable<Timelines> {
        self.inner.timelines.read_only()
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

pub trait AssetExt<V>: 'static + Send + Sync {
    fn load(&self, assets: &AssetCache) -> anyhow::Result<Asset<V>>;
}

impl<T> AssetExt<T::Output> for T
where
    T: AssetDesc,
    T::Error: Into<anyhow::Error>,
{
    fn load(&self, assets: &AssetCache) -> anyhow::Result<Asset<T::Output>> {
        assets.try_load(self).map_err(Into::into)
    }
}

pub trait AsyncAssetExt<V>: 'static + Send + Sync {
    fn load_async(&self, assets: &AssetCache) -> AssetLoadFuture<V>;
}

impl<T, V> AsyncAssetExt<V> for T
where
    T: AsyncAssetKey<Output = V>,
    T::Error: Debug + Display,
    V: 'static + Send + Sync,
{
    fn load_async(&self, assets: &AssetCache) -> AssetLoadFuture<V> {
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
pub trait AssetDesc: StoredKey + Debug {
    type Output: 'static + Send + Sync;
    type Error: 'static + Debug;

    fn create(&self, assets: &AssetCache) -> Result<Asset<Self::Output>, Self::Error>;

    fn label(&self) -> String {
        format!("{self:?}")
    }
}

/// Describes a description that allows loading an asset.
///
/// Assets are immutable, and shared.
///
/// For mutable and owned/exclusive resource loading, refer to [`Loadable`]
pub trait AsyncAssetKey: StoredKey + Debug + Send + Sync {
    type Output: 'static + Send + Sync;
    type Error: Send + Sync + 'static + Debug + Display + Into<anyhow::Error>;

    /// Create the resource
    ///
    /// Returns Asset directly to allow returning already loaded assets stored elsewhere
    fn create(
        &self,
        assets: &AssetCache,
    ) -> impl Future<Output = Result<Asset<Self::Output>, Self::Error>> + Send;

    fn label(&self) -> String {
        format!("{self:?}")
    }
}

/// We implement `LoadFromPath` for `DynamicImage`, which causes `AssetPath<DynamicImage>` to be
/// an `AsyncAssetDesc` that can be used to load images and cache images from the filesystem.
// impl LoadFromPath for DynamicImage {
//     type Error = anyhow::Error;

//     async fn load(path: AssetPath<Self>, assets: &AssetCache) -> anyhow::Result<Self> {
//         let format = image::ImageFormat::from_path(path.path())?;
//         let data = assets
//             .service::<FileSystemMapService>()
//             .load_bytes_async(path.path())
//             .await?;

//         let image = async_std::task::spawn_blocking(move || {
//             profile_scope!("load_image_blocking");
//             image::load_from_memory_with_format(&data, format)
//         })
//         .await?;
//         Ok(image)
//     }
// }

impl LoadFromPath for DynamicImage {
    async fn load_from_file(
        path: AssetPath<DynamicImage>,
        assets: &AssetCache,
    ) -> Result<Self, anyhow::Error> {
        let format = image::ImageFormat::from_path(path.path())?;
        let data = assets
            .service::<FileSystemMapService>()
            .load_bytes_async(path.path())
            .await?;

        let image = async_std::task::spawn_blocking(move || {
            profile_scope!("load_image_blocking");
            image::load_from_memory_with_format(&data, format)
        })
        .await?;
        Ok(image)
    }
}

type SharedLoadFuture<T> = Shared<BoxFuture<'static, Result<Asset<T>, SharedError>>>;

pub struct AssetLoadFuture<T> {
    inner: Result<Result<Asset<T>, SharedError>, SharedLoadFuture<T>>,
}

impl<T> AssetLoadFuture<T> {
    /// Returns the value if loaded
    /// Can be called multiple times until loaded
    pub fn try_get(&self) -> Option<Result<Asset<T>, SharedError>> {
        match &self.inner {
            Ok(v) => Some(v.clone()),
            Err(v) => v.peek().cloned(),
        }
    }
}

impl<T> Future for AssetLoadFuture<T> {
    type Output = Result<Asset<T>, SharedError>;

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
    use std::convert::Infallible;

    use super::*;
    use crate::service::FsAssetError;

    #[test]
    fn asset_cache() {
        struct TestAsset;

        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        struct TestAssetKey(String);

        impl AssetDesc for TestAssetKey {
            type Output = TestAsset;
            type Error = FsAssetError;

            fn create(&self, assets: &AssetCache) -> Result<Asset<Self::Output>, Self::Error> {
                eprintln!("Loading {:?}", self.0);
                Ok(assets.insert(TestAsset))
            }
        }

        let assets = AssetCache::new();

        let content: Asset<TestAsset> = assets.load(&TestAssetKey("Foo".into()));
        let content2: Asset<TestAsset> = assets.load(&TestAssetKey("Foo".into()));
        let bar: Asset<TestAsset> = assets.load(&TestAssetKey("Bar".into()));
        let content4: Asset<TestAsset> = assets.load(&TestAssetKey("Foo".into()));

        assert_eq!(content, content2);

        assert!(Arc::ptr_eq(content.as_arc(), content2.as_arc()));
        assert!(!Arc::ptr_eq(content.as_arc(), bar.as_arc()));
        assert_eq!(content, content4);

        assert!(assets.get(&TestAssetKey("Bar".to_string())).is_some());

        drop(bar);

        assert!(assets.get(&TestAssetKey("Bar".to_string())).is_none());
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

        #[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        struct Key(String);

        impl AsyncAssetKey for Key {
            type Output = ();
            type Error = Infallible;

            async fn create(&self, assets: &AssetCache) -> Result<Asset<()>, Infallible> {
                eprintln!("Loading {self:?}");
                YieldOnce { yielded: false }.await;

                eprintln!("Finished {self:?}");
                Ok(assets.insert(()))
            }
        }

        let assets = AssetCache::new();

        eprintln!("Starting first request");
        let mut pending1 = Box::pin(assets.try_load_async(&Key("Foo".into())));

        eprintln!("Starting second request");

        let pending2 = assets.try_load_async(&Key("Foo".into()));

        use futures::future::FutureExt;
        assert!((&mut pending1).now_or_never().is_none());
        let content2 = pending2.now_or_never().unwrap().unwrap();
        let content = pending1.now_or_never().unwrap().unwrap();
        assert!(Arc::ptr_eq(content.as_arc(), content2.as_arc()));

        let pending3 = assets
            .try_load_async(&Key("Foo".into()))
            .now_or_never()
            .unwrap()
            .unwrap();
        assert!(Arc::ptr_eq(content.as_arc(), pending3.as_arc()));
    }
}
