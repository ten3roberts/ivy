use std::any;

use anymap::AnyMap;
use atomic_refcell::{AtomicRef, AtomicRefCell, AtomicRefMut};

use crate::{Error, Handle, ResourceCache};

/// A ResourceManager abstracts and holds ResourceCaches for many different types. This is to avoid
/// having to individually keep track of all Texture, Sampler, Material and other cache and wrap
/// each in a RefCell or Rc.

pub struct ResourceManager {
    caches: AnyMap,
}

impl ResourceManager {
    /// Creates a new empty resource manager. Caches will be added when requested.
    pub fn new() -> Self {
        Self {
            caches: AnyMap::new(),
        }
    }

    /// Returns a reference to ResourceCache for type `T`
    pub fn cache<T: 'static>(&self) -> Result<AtomicRef<ResourceCache<T>>, Error> {
        self.caches
            .get::<AtomicRefCell<ResourceCache<T>>>()
            .ok_or_else(|| Error::MissingCache(any::type_name::<T>()))?
            .try_borrow()
            .map_err(|_| Error::Borrow(any::type_name::<T>()))
    }

    /// Returns a mutable reference to ResourceCache for type `T`
    pub fn cache_mut<T: 'static>(&self) -> Result<AtomicRefMut<ResourceCache<T>>, Error> {
        self.caches
            .get::<AtomicRefCell<ResourceCache<T>>>()
            .ok_or_else(|| Error::MissingCache(any::type_name::<T>()))?
            .try_borrow_mut()
            .map_err(|_| Error::BorrowMut(any::type_name::<T>()))
    }

    /// Returns a reference to the resource pointed to by Handle<T>. Equivalent to using `cache()` and then `get()`. If dereferencing many handles, prefer gettting the cache first and the using it directly.
    pub fn get<T: 'static>(&self, handle: Handle<T>) -> Result<AtomicRef<T>, Error> {
        let cache = self.cache::<T>()?;

        AtomicRef::filter_map(cache, |cache| cache.get(handle).ok())
            .ok_or_else(|| Error::InvalidHandle(any::type_name::<T>()))
    }

    /// Returns a mutable reference to the resource pointed to by Handle<T>. Equivalent to using `cache()` and then `get()`. If dereferencing many handles, prefer gettting the cache first and the using it directly.
    pub fn get_mut<T: 'static>(&self, handle: Handle<T>) -> Result<AtomicRefMut<T>, Error> {
        let cache = self.cache_mut::<T>()?;

        AtomicRefMut::filter_map(cache, |cache| cache.get_mut(handle).ok())
            .ok_or_else(|| Error::InvalidHandle(any::type_name::<T>()))
    }

    /// Inserts a resource into the correct cache and returns a handle to acces the resource.
    /// Fails if the cache does not exists.
    pub fn insert<T: 'static>(&self, resource: T) -> Result<Handle<T>, Error> {
        let mut cache = self.cache_mut::<T>()?;

        Ok(cache.insert(resource))
    }

    /// Creates a resource cache for `T`. Does nothing if cache already exists.
    pub fn create_cache<T: 'static>(&mut self) {
        if self
            .caches
            .get::<AtomicRefCell<ResourceCache<T>>>()
            .is_none()
        {
            self.caches
                .insert(AtomicRefCell::new(ResourceCache::<T>::new()));
        }
    }
}

impl Default for ResourceManager {
    fn default() -> Self {
        Self::new()
    }
}
