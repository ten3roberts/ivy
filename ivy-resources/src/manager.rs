use crate::{
    cell::{Cell, CellRef, CellRefMut, Storage},
    Result,
};
use std::{any::TypeId, collections::HashMap};

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
    pub fn fetch<T: Storage>(&self) -> Result<ResourceView<T>> {
        // A raw pointer to the internally boxed AtomicRefCell is acquired. The pointer is
        // valid as long as self because caches can never be individually removed
        match self.caches.read().get(&TypeId::of::<T>()) {
            Some(val) => return val.borrow(),
            None => {}
        }

        // Insert new cache
        self.create_cache::<T>().borrow()
    }

    /// Returns a mutable cache containing all resources of type `T`. Use this if accessing many
    /// resources of
    /// the same type to avoid having to lookup the entry multiple times.
    ///
    /// Fails if the type is already borrowed.
    pub fn fetch_mut<T: Storage>(&self) -> Result<ResourceViewMut<T>> {
        // A raw pointer to the internally boxed AtomicRefCell is acquired. The pointer is
        // valid as long as self because caches can never be individually removed
        match self.caches.read().get(&TypeId::of::<T>()) {
            Some(val) => return val.borrow_mut(),
            None => {}
        }

        // Insert new cache
        self.create_cache::<T>().borrow_mut()
    }

    /// Returns a reference to the resource pointed to by Handle<T>. Equivalent to using `cache()`
    /// and then `get()`. If dereferencing many handles, prefer gettting the cache first and the using
    /// it directly.
    ///
    /// Fails if the type is already mutably borrowed.
    pub fn get<T: Storage>(&self, handle: Handle<T>) -> Result<CellRef<T>> {
        let cache = self.fetch::<T>()?;

        cache.try_map(|cache| cache.get(handle))
    }

    /// Returns a mutable reference to the resource pointed to by Handle<T>. Equivalent to using
    /// `cache()` and then `get()`. If dereferencing many handles, prefer gettting the cache first and
    /// the using it directly.
    ///
    /// Fails if the type is already borrowed.
    pub fn get_mut<T: Storage>(&self, handle: Handle<T>) -> Result<CellRefMut<T>> {
        let cache = self.fetch_mut::<T>()?;

        cache.try_map(|cache| cache.get_mut(handle))
    }

    /// Inserts a resource into the correct cache and returns a handle to acces the resource.
    ///
    /// Fails if the type is already mutably borrowed.
    pub fn insert<T: Storage>(&self, resource: T) -> Result<Handle<T>> {
        self.fetch_mut().map(|mut val| val.insert(resource))
    }

    fn create_cache<T: Storage>(&self) -> MappedRwLockWriteGuard<Cell> {
        RwLockWriteGuard::map(self.caches.write(), |guard| {
            guard
                .entry(TypeId::of::<T>())
                .or_insert_with(|| Cell::new(ResourceCache::<T>::new()))
        })
    }
}

impl Default for Resources {
    fn default() -> Self {
        Self::new()
    }
}
