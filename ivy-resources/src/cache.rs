use crate::Result;
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
        Self {
            slots: SlotMap::with_key(),
            default: Handle::null(),
        }
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

    /// Returns the resource by handle, or the default is the handle is invalid.
    /// Note: The function still may fail to acquire a resource if the default is null
    pub fn get_or_default_mut(&mut self, handle: Handle<T>) -> Result<&mut T> {
        if self.slots.contains_key(handle) {
            Ok(self.slots.get_mut(handle).unwrap())
        } else {
            self.get_default_mut()
        }
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

impl<T: 'static + Sized> Default for ResourceCache<T> {
    fn default() -> Self {
        Self::new()
    }
}
