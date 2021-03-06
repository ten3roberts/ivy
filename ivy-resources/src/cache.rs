use crate::{entry::Entry, CellRefMut, HandleWrapper, LoadResource, Resources, Result};
use std::any::type_name;

use slotmap::SlotMap;

use crate::{Error, Handle};

pub trait Resource {}

/// Stores resources of a single type. Resources are inserted and accessed by a handle.
/// There is also a default handle that is associated to the first inserted resource, unless
/// changed.
pub struct ResourceCache<T> {
    slots: SlotMap<Handle<T>, T>,
    default: Handle<T>,
}

impl<T: 'static + Sized> ResourceCache<T> {
    pub fn new() -> Self {
        Default::default()
    }

    // Inserts a new resource into the cache.
    // If the cache is empty, the default is set to the first inserted value
    #[inline]
    pub fn insert(&mut self, value: T) -> Handle<T> {
        let handle = self.slots.insert(value);

        // Set the default to the first inserted value
        if self.slots.len() == 1 {
            self.default = handle;
        }

        handle
    }

    /// Free and invalidate all handles
    pub fn clear(&mut self) {
        self.slots.clear();
        self.default = Handle::null();
    }
    // Inserts a new resource into the cache.
    // If the cache is empty, the default is set to the first inserted value.
    // Returns a reference to the new value.
    #[inline]
    pub fn insert_get(&mut self, value: T) -> HandleWrapper<T, &mut T> {
        let handle = self.slots.insert(value);

        // Set the default to the first inserted value
        if self.slots.len() == 1 {
            self.default = handle;
        }

        HandleWrapper {
            handle,
            borrow: &mut self.slots[handle],
        }
    }

    // Inserts a new resource into the cache.
    // If the cache is empty, the default is set to the first inserted value.
    // Returns a reference to the new value.
    #[inline]
    pub fn insert_get_cell(cache: CellRefMut<Self>, value: T) -> HandleWrapper<T, CellRefMut<T>> {
        let mut handle = Default::default();
        let borrow = cache.map(|cache| {
            handle = cache.slots.insert(value);

            // Set the default to the first inserted value
            if cache.slots.len() == 1 {
                cache.default = handle;
            }

            &mut cache.slots[handle]
        });

        HandleWrapper { handle, borrow }
    }

    // Inserts a new resource into the cache and marks it as the default
    #[inline]
    pub fn insert_default(&mut self, value: T) -> Handle<T> {
        self.default = self.slots.insert(value);
        self.default
    }

    #[inline]
    pub fn get(&self, handle: Handle<T>) -> Result<&T> {
        if handle.is_null() {
            return Err(Error::NullHandle(type_name::<T>()));
        }

        self.slots
            .get(handle)
            .ok_or_else(|| Error::InvalidHandle(type_name::<T>()))
    }

    #[inline]
    pub fn get_mut(&mut self, handle: Handle<T>) -> Result<&mut T> {
        if handle.is_null() {
            return Err(Error::NullHandle(type_name::<T>()));
        }

        self.slots
            .get_mut(handle)
            .ok_or_else(|| Error::InvalidHandle(type_name::<T>()))
    }

    /// Returns the resource by handle, or the default is the handle is invalid.
    /// Note: The function still may fail to acquire a resource if the default is null
    #[inline]
    pub fn get_or_default(&self, handle: Handle<T>) -> Result<&T> {
        self.get(handle).or_else(|_| self.get_default())
    }

    /// Mimics the entry api of HashMap.
    pub fn entry(&mut self, handle: Handle<T>) -> Entry<T> {
        if self.slots.get(handle).is_some() {
            Entry::Occupied(self, handle)
        } else {
            Entry::Vacant(self)
        }
    }

    /// Entry api for the default key if it may or may not exist.
    pub fn default_entry(&mut self) -> Entry<T> {
        self.entry(self.default)
    }

    // Returns the handle to the default resource stored in this cache.
    // The handle is null if the cache is empty.
    #[inline]
    pub fn default_handle(&self) -> Handle<T> {
        self.default
    }

    // Returns the current default resource
    #[inline]
    pub fn get_default(&self) -> Result<&T> {
        self.get(self.default)
            .map_err(|_| Error::MissingDefault(type_name::<T>()))
    }

    // Returns a mutable reference to the default resource
    #[inline]
    pub fn get_default_mut(&mut self) -> Result<&mut T> {
        self.get_mut(self.default)
            .map_err(|_| Error::MissingDefault(type_name::<T>()))
    }

    // Sets the default resource.
    // Pass Handle::null to remove the default.
    #[inline]
    pub fn set_default(&mut self, handle: Handle<T>) {
        self.default = handle;
    }

    #[inline]
    pub fn default(&self) -> Handle<T> {
        self.default
    }
}

impl<T, I, E> ResourceCache<T>
where
    T: 'static + LoadResource<Info = I, Error = E>,
{
    /// Attempts to load and insert a resource from the given create info. If
    /// info from the same info already exists, it will be returned. This means
    /// the load function has to be injective over `info`.
    pub fn load(&mut self, resources: &Resources, info: &I) -> std::result::Result<Handle<T>, E> {
        let resource = T::load(resources, info)?;
        Ok(self.insert(resource))
    }
}

impl<T: 'static + Sized> Default for ResourceCache<T> {
    fn default() -> Self {
        Self {
            slots: SlotMap::with_key(),
            // info_map: HashMap::new(),
            default: Handle::null(),
        }
    }
}
