use crate::{
    cell::{Cell, CellRef, CellRefMut, Storage},
    LoadResource, RefEntry, Result,
};
use std::{
    any::{type_name, TypeId},
    collections::{hash_map::Entry, HashMap},
};

use parking_lot::{MappedRwLockWriteGuard, RwLock, RwLockWriteGuard};

use crate::{Handle, ResourceCache};

pub type ResourceView<'a, T> = CellRef<'a, ResourceCache<T>>;
pub type ResourceViewMut<'a, T> = CellRefMut<'a, ResourceCache<T>>;

/// ResourceManager is a container for multi-valued strongly typed assets.
/// Any static Send + Sync type can be stored in the container and a handle is returned to access
/// the value using interior mutability. Containers for each used type is automatically
/// created.
pub struct Resources {
    caches: RwLock<HashMap<TypeId, Cell>>,
}

impl Resources {
    /// Creates a new empty resource manager. Caches will be added when requested.
    pub fn new() -> Self {
        Self {
            caches: RwLock::new(HashMap::new()),
        }
    }

    /// Returns a cache containing all resources of type `T`. Use this if accessing many
    /// resources of
    /// the same type to avoid having to lookup the entry multiple times.
    ///
    /// Fails if the type is already mutably borrowed.
    #[inline]
    pub fn fetch<T: Storage>(&self) -> Result<ResourceView<T>> {
        // A raw pointer to the internally boxed AtomicRefCell is acquired. The pointer is
        // valid as long as self because caches can never be individually removed
        if let Some(val) = self.caches.read().get(&TypeId::of::<T>()) {
            return val.borrow();
        }

        // Insert new cache
        self.create_cache::<T>().borrow()
    }

    /// Returns a mutable cache containing all resources of type `T`. Use this if accessing many
    /// resources of
    /// the same type to avoid having to lookup the entry multiple times.
    ///
    /// Fails if the type is already borrowed.
    #[inline]
    pub fn fetch_mut<T: Storage>(&self) -> Result<ResourceViewMut<T>> {
        // A raw pointer to the internally boxed AtomicRefCell is acquired. The pointer is
        // valid as long as self because caches can never be individually removed
        if let Some(val) = self.caches.read().get(&TypeId::of::<T>()) {
            return val.borrow_mut();
        }

        // Insert new cache
        self.create_cache::<T>().borrow_mut()
    }

    /// Returns a reference to the resource pointed to by Handle<T>. Equivalent to using `cache()`
    /// and then `get()`. If dereferencing many handles, prefer gettting the cache first and the using
    /// it directly.
    ///
    /// Fails if the type is already mutably borrowed.
    #[inline]
    pub fn get<T: Storage>(&self, handle: Handle<T>) -> Result<CellRef<T>> {
        self.fetch::<T>()?.try_map(|cache| cache.get(handle))
    }

    /// Returns the resource by handle, or the default is the handle is invalid.
    /// Note: The function still may fail to acquire a resource if the default is null
    #[inline]
    pub fn get_or_default<T: Storage>(&self, handle: Handle<T>) -> Result<CellRef<T>> {
        self.fetch::<T>()?
            .try_map(|cache| cache.get_or_default(handle))
    }

    /// Mimics the entry api of HashMap.
    pub fn entry<T: Storage>(&self, handle: Handle<T>) -> Result<RefEntry<T>> {
        let cache = self.fetch_mut()?;
        if cache.get(handle).is_ok() {
            Ok(RefEntry::Occupied(cache, handle))
        } else {
            Ok(RefEntry::Vacant(cache))
        }
    }

    /// Entry api for the default key if it may or may not exist.
    pub fn default_entry<T: Storage>(&self) -> Result<RefEntry<T>> {
        let cache = self.fetch_mut()?;
        let default = cache.default();
        if cache.get(default).is_ok() {
            Ok(RefEntry::Occupied(cache, default))
        } else {
            Ok(RefEntry::Vacant(cache))
        }
    }

    /// Returns a mutable reference to the resource pointed to by Handle<T>. Equivalent to using
    /// `cache()` and then `get()`. If dereferencing many handles, prefer gettting the cache first and
    /// the using it directly.
    ///
    /// Fails if the type is already borrowed.
    #[inline]
    pub fn get_mut<T: Storage>(&self, handle: Handle<T>) -> Result<CellRefMut<T>> {
        self.fetch_mut::<T>()?
            .try_map(|cache| cache.get_mut(handle))
    }

    // Returns the current default resource for T.
    #[inline]
    pub fn get_default<T: Storage>(&self) -> Result<CellRef<T>> {
        self.fetch::<T>()?.try_map(|cache| cache.get_default())
    }

    // Returns the current default resource for T.
    #[inline]
    pub fn get_default_mut<T: Storage>(&self) -> Result<CellRefMut<T>> {
        self.fetch_mut::<T>()?
            .try_map(|cache| cache.get_default_mut())
    }

    // Sets the default resource.
    // Pass Handle::null to remove the default.
    #[inline]
    pub fn set_default<T: Storage>(&self, handle: Handle<T>) -> Result<()> {
        self.fetch_mut::<T>()
            .map(|mut cache| cache.set_default(handle))
    }

    #[inline]
    pub fn default<T: Storage>(&self) -> Result<Handle<T>> {
        self.fetch::<T>().map(|cache| cache.default())
    }

    /// Inserts a resource into the correct cache and returns a handle to acces the resource.
    ///
    /// Fails if the type is already mutably borrowed.
    #[inline]
    pub fn insert<T: Storage>(&self, resource: T) -> Result<Handle<T>> {
        self.fetch_mut().map(|mut val| val.insert(resource))
    }

    /// Inserts a resource into the correct cache and marks it as default.
    ///
    /// Fails if the type is already mutably borrowed.
    #[inline]
    pub fn insert_default<T: Storage>(&self, resource: T) -> Result<Handle<T>> {
        self.fetch_mut().map(|mut val| val.insert_default(resource))
    }

    fn create_cache<T: Storage>(&self) -> MappedRwLockWriteGuard<Cell> {
        RwLockWriteGuard::map(self.caches.write(), |guard| {
            guard
                .entry(TypeId::of::<T>())
                .or_insert_with(|| Cell::new(ResourceCache::<T>::new()))
        })
    }

    /// Attempts to load and insert a resource from the given create info. If
    /// info from the same info already exists, it will be returned. This means
    /// the load function has to be injective over `info`.
    pub fn load<T, I, E, G>(&self, info: G) -> Result<std::result::Result<Handle<T>, E>>
    where
        G: Into<I>,
        I: std::hash::Hash + Eq + Storage,
        T: Storage + LoadResource<Info = I, Error = E>,
    {
        let mut info_cache: CellRefMut<InfoCache<I, T>> = self
            .default_entry()?
            .or_insert_with(|| InfoCache(HashMap::new()));

        let info = info.into();

        match info_cache.0.entry(info) {
            Entry::Occupied(entry) => {
                println!("Deduplicated: {:?}", type_name::<T>());
                Ok(Ok(*entry.get()))
            }
            Entry::Vacant(entry) => {
                let val = match self.fetch_mut::<T>()?.load(self, entry.key()) {
                    Ok(val) => val,
                    Err(e) => return Ok(Err(e)),
                };
                Ok(Ok(*entry.insert(val)))
            }
        }
    }

    /// Attempts to load and insert a resource from the given create info.
    pub fn load_uncached<T, I, E, G>(&self, info: G) -> Result<std::result::Result<Handle<T>, E>>
    where
        G: Into<I>,
        T: Storage + LoadResource<Info = I, Error = E>,
    {
        let info = info.into();

        self.fetch_mut::<T>().map(|mut val| val.load(self, &info))
    }
}

impl Default for Resources {
    fn default() -> Self {
        Self::new()
    }
}

struct InfoCache<I, T>(HashMap<I, Handle<T>>);
